#!/usr/bin/env python3
"""feishu.py 对着一个假的飞书跑：python3 test_feishu.py

核对的是脚本发出的请求，不是飞书本身。
"""

import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))


class Feishu(BaseHTTPRequestHandler):
    calls = []

    def log_message(self, *_):
        pass

    def answer(self, body):
        data = json.dumps(body).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        url = urllib.parse.urlparse(self.path)
        query = urllib.parse.parse_qs(url.query)
        Feishu.calls.append((url.path, query, None, self.headers.get("Authorization")))
        if url.path == "/open-apis/search/v1/user" and self.headers.get("Authorization") == "Bearer access-2":
            if query.get("query") == ["未授权"]:
                return self.answer({"code": 99991679, "msg": "Unauthorized"})
            people = [{"name": "吴诺 Uno", "open_id": "ou_uno"}, {"name": "外人"}] if query.get("query") == ["uno"] else []
            return self.answer({"code": 0, "data": {"users": people, "has_more": False}})
        return self.answer({"code": 404, "msg": "no such route"})

    def do_POST(self):
        url = urllib.parse.urlparse(self.path)
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length") or 0)) or b"{}")
        Feishu.calls.append((url.path, urllib.parse.parse_qs(url.query), body, self.headers.get("Authorization")))
        if url.path == "/open-apis/authen/v2/oauth/token":
            ok = body.get("client_id") == "cli_app" and body.get("client_secret") == "s3cret" and body.get("refresh_token") == "refresh-1"
            if not ok:
                return self.answer({"error": "invalid_grant"})
            return self.answer({"access_token": "access-2", "expires_in": 7200, "refresh_token": "refresh-2"})
        if self.headers.get("Authorization") != "Bearer access-2":
            return self.answer({"code": 99991663, "msg": "token invalid"})
        if url.path == "/open-apis/im/v1/messages":
            if body.get("receive_id") == "nobody@x.com":
                return self.answer({"code": 230001, "msg": "invalid receive_id"})
            return self.answer({"code": 0, "data": {"message_id": "om_new"}})
        return self.answer({"code": 404, "msg": "no such route"})


class Plugin(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), Feishu)
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    def setUp(self):
        Feishu.calls = []
        self.data = tempfile.mkdtemp()
        self.env = dict(os.environ, SOPHON_PLUGIN_DATA=self.data, FEISHU_OPEN_BASE="http://127.0.0.1:%d" % self.server.server_port,
                        SOPHON_PARAM_APP_ID="cli_app", SOPHON_PARAM_APP_SECRET="s3cret")

    def sign_in(self, expires_in=7200):
        user = {"access_token": "access-2" if expires_in > 300 else "access-1", "refresh_token": "refresh-1",
                "expires_at": time.time() + expires_in, "name": "阿达", "open_id": "ou_me"}
        with open(os.path.join(self.data, "user.json"), "w") as file:
            json.dump(user, file)

    def run_plugin(self, *args):
        result = subprocess.run([sys.executable, os.path.join(HERE, "feishu.py"), *args], env=self.env, capture_output=True, text=True, timeout=30)
        return result.returncode, result.stdout, result.stderr

    def test_status_says_who_or_fails(self):
        self.assertEqual(self.run_plugin("status")[0], 1)
        self.sign_in()
        self.assertEqual(self.run_plugin("status")[:2], (0, "已授权：阿达\n"))

    def test_send_goes_out_as_me_to_an_email(self):
        # 令牌快过期：先换新的再发。
        self.sign_in(expires_in=60)
        code, out, err = self.run_plugin("send", "--email", "ada@x.com", "--text", "🤖 已评论\nhttps://github.com/acme/app/pull/12")
        self.assertEqual((code, out), (0, "已发送\n"), err)
        path, query, body, token = Feishu.calls[-1]
        self.assertEqual((path, query, token), ("/open-apis/im/v1/messages", {"receive_id_type": ["email"]}, "Bearer access-2"))
        self.assertEqual(body["receive_id"], "ada@x.com")
        self.assertEqual(body["msg_type"], "text")
        self.assertEqual(json.loads(body["content"]), {"text": "🤖 已评论\nhttps://github.com/acme/app/pull/12"})

    def test_send_to_a_chat_or_an_open_id(self):
        self.sign_in()
        self.assertEqual(self.run_plugin("send", "--chat", "oc_1", "--text", "hi")[0], 0)
        self.assertEqual(Feishu.calls[-1][1], {"receive_id_type": ["chat_id"]})
        self.assertEqual(self.run_plugin("send", "--open-id", "ou_2", "--text", "hi")[0], 0)
        self.assertEqual(Feishu.calls[-1][1], {"receive_id_type": ["open_id"]})

    def test_send_says_why_it_failed(self):
        self.sign_in()
        code, out, err = self.run_plugin("send", "--email", "nobody@x.com", "--text", "hi")
        self.assertEqual((code, out), (1, ""))
        self.assertIn("invalid receive_id", err)
        self.assertEqual(self.run_plugin("send", "--text", "hi")[0], 1)
        self.assertEqual(self.run_plugin("send", "--email", "a@x.com")[0], 1)

    def test_find_lists_who_is_called_that(self):
        self.sign_in()
        code, out, err = self.run_plugin("find", "uno")
        self.assertEqual((code, json.loads(out)), (0, {"name": "吴诺 Uno", "open_id": "ou_uno"}), err)
        self.assertEqual(Feishu.calls[-1][:2], ("/open-apis/search/v1/user", {"query": ["uno"], "page_size": ["10"]}))
        # 没有这个人：不算失败，什么也不列。
        code, out, err = self.run_plugin("find", "nobody")
        self.assertEqual((code, out), (0, ""))
        self.assertIn("没有找到", err)
        # 权限还没开通：说清要做什么。
        code, out, err = self.run_plugin("find", "未授权")
        self.assertEqual(code, 1)
        self.assertIn("contact:user:search", err)
        self.assertEqual(self.run_plugin("find")[0], 1)


if __name__ == "__main__":
    unittest.main()
