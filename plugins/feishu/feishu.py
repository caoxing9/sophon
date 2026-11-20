#!/usr/bin/env python3
"""飞书：Sophon 的集成。用你本人的飞书身份登录一次，之后自动化的脚本和
自动化里的 AI 可以通过它发消息：

    sophon use feishu send --email someone@company.com --text "…"
    sophon use feishu send --open-id ou_xxx --text "…"
    sophon use feishu send --chat oc_xxx --text "…"
    sophon use feishu find 名字            按名字在飞书里找同事：每行一个人的名字和 open_id

    feishu.py login    用你的飞书账号授权一次（Sophon 的插件页里点「授权」）
    feishu.py status   授权的是谁；还没有授权就失败

凭据和令牌只留在这个插件自己的数据目录里。只用 Python 标准库。
"""

import fcntl
import html
import json
import os
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

DATA = os.environ.get("SOPHON_PLUGIN_DATA") or os.path.expanduser("~/.sophon-feishu")
PORT = int(os.environ.get("FEISHU_REDIRECT_PORT") or 17635)
REDIRECT = "http://127.0.0.1:%d/feishu/oauth" % PORT
# 应用里要作为「用户身份权限」开通的权限。
SCOPES = [
    "im:message",
    "im:message.send_as_user",
    "im:message.group_msg:get_as_user",
    "im:message.p2p_msg:get_as_user",
    "search:message",
    "contact:user:search",
    "offline_access",
]


class Fail(Exception):
    pass


# ---------------------------------------------------------------- 本地文件


def path(name):
    return os.path.join(DATA, name)


def load(name, default=None):
    try:
        with open(path(name), encoding="utf-8") as file:
            return json.load(file)
    except (OSError, ValueError):
        return default


def save(name, value):
    os.makedirs(DATA, mode=0o700, exist_ok=True)
    temporary = path(name + ".tmp")
    with os.fdopen(os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w", encoding="utf-8") as file:
        json.dump(value, file, ensure_ascii=False)
    os.replace(temporary, path(name))


class Locked:
    """一次只让一个进程换令牌：刷新令牌只能用一次，探测和回话会同时跑。"""

    def __enter__(self):
        os.makedirs(DATA, mode=0o700, exist_ok=True)
        self.file = open(path("lock"), "w")
        fcntl.flock(self.file, fcntl.LOCK_EX)

    def __exit__(self, *_):
        fcntl.flock(self.file, fcntl.LOCK_UN)
        self.file.close()


# ---------------------------------------------------------------- 飞书接口


def settings(complete=True):
    """飞书应用和仓库范围：插件的参数（分享的人预填的，或你在这台电脑上改的），
    没有参数时用 login 存下的。"""
    config = load("config.json") or {}
    for name in ("app_id", "app_secret", "domain"):
        value = (os.environ.get("SOPHON_PARAM_" + name.upper()) or "").strip()
        if value:
            config[name] = value
    if complete and (not config.get("app_id") or not config.get("app_secret")):
        raise Fail("还没有飞书应用的 App ID 和 App Secret：在 Sophon 的「集成」页里填写")
    return config


def bases(config):
    lark = config.get("domain") == "lark"
    opened = os.environ.get("FEISHU_OPEN_BASE") or ("https://open.larksuite.com" if lark else "https://open.feishu.cn")
    accounts = os.environ.get("FEISHU_ACCOUNTS_BASE") or (
        "https://accounts.larksuite.com" if lark else "https://accounts.feishu.cn"
    )
    return opened, accounts


def http(method, url, token=None, body=None, params=None):
    """一次请求，回 JSON；飞书说失败（HTTP 错误，或 code 不为 0）就是 Fail。"""
    if params:
        url += "?" + urllib.parse.urlencode({key: value for key, value in params.items() if value is not None})
    headers = {}
    if token:
        headers["Authorization"] = "Bearer " + token
    data = None
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json; charset=utf-8"
    request = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            text = response.read().decode("utf-8")
    except urllib.error.HTTPError as error:
        text = error.read().decode("utf-8", "replace")
    except OSError as error:
        raise Fail("连不上飞书：%s" % error)
    try:
        answer = json.loads(text)
    except ValueError:
        raise Fail("飞书 %s 的回答读不懂：%s" % (urllib.parse.urlparse(url).path, text[:160]))
    return answer


def api(method, route, token, body=None, params=None):
    config = settings()
    answer = http(method, bases(config)[0] + route, token, body, params)
    if answer.get("code") not in (0, None):
        raise Fail("飞书 %s：%s %s" % (route, answer.get("code"), answer.get("msg") or ""))
    return answer.get("data") or {}


def exchange(config, grant, previous=None):
    """拿授权码或刷新令牌换一份新令牌，存下并返回。"""
    opened = bases(config)[0]
    body = dict(grant, client_id=config["app_id"], client_secret=config["app_secret"])
    answer = http("POST", opened + "/open-apis/authen/v2/oauth/token", body=body)
    if not answer.get("access_token"):
        raise Fail(
            "换令牌失败：%s"
            % (answer.get("error_description") or answer.get("error") or answer.get("msg") or answer.get("code"))
        )
    who = None
    if previous and grant.get("grant_type") == "refresh_token":
        who = {"name": previous.get("name"), "open_id": previous.get("open_id")}
    if not who or not who.get("open_id"):
        info = http("GET", opened + "/open-apis/authen/v1/user_info", answer["access_token"]).get("data") or {}
        if not info.get("open_id"):
            raise Fail("读不到授权的是谁")
        who = {"name": info.get("name") or info["open_id"], "open_id": info["open_id"]}
    now = time.time()
    user = {
        "access_token": answer["access_token"],
        "refresh_token": answer.get("refresh_token") or (previous or {}).get("refresh_token"),
        "expires_at": now + (answer.get("expires_in") or 7200),
        "refresh_expires_at": (now + answer["refresh_token_expires_in"])
        if answer.get("refresh_token_expires_in")
        else (previous or {}).get("refresh_expires_at"),
    }
    user.update(who)
    save("user.json", user)
    return user


def signed_in():
    """你的令牌和 open_id；剩不到五分钟就先换新的。"""
    config = settings()
    with Locked():
        user = load("user.json")
        if not user or not user.get("access_token"):
            raise Fail("还没有授权：在 Sophon 的「集成」页里点「授权」")
        if user.get("expires_at", 0) - time.time() > 300:
            return user
        if not user.get("refresh_token"):
            raise Fail("授权过期了：在 Sophon 的「集成」页里重新点「授权」")
        return exchange(config, {"grant_type": "refresh_token", "refresh_token": user["refresh_token"]}, user)


# ---------------------------------------------------------------- login


# 授权回调页：浏览器里只出现这一次。颜色跟 Sophon 的主题（theme.json）一致；
# 成功时头像方块落下、对勾徽章弹出，倒数几秒后页面自己关（浏览器不让关就改口
# 请人手动关）。样式和唯一的一段脚本都内联，脚本带 nonce，名字和错误信息一律
# 转义后再放进去。
CLOSE_SECONDS = 3
SPRING = "cubic-bezier(.34,1.56,.64,1)"
EASE = "cubic-bezier(.2,.8,.2,1)"
PAGE = """<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sophon · 飞书</title>
<style>
:root{color-scheme:light dark;--bg:#faf9f5;--card:#fdfcf9;--fg:#1f1e1d;--muted:#6b6960;--border:#e6e3d9;--tile:#f0eee6;--ok:#c96442;--bad:#b53333;--bad-soft:#f6dfdc}
@media (prefers-color-scheme:dark){:root{--bg:#262624;--card:#2a2a27;--fg:#f5f4ee;--muted:#a6a39a;--border:#3a3a37;--tile:#30302e;--ok:#d97757;--bad:#e0705f;--bad-soft:#4a2420}}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;background:var(--bg);color:var(--fg);
font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Hiragino Sans GB","Microsoft YaHei","Segoe UI",sans-serif;-webkit-font-smoothing:antialiased}
.card{position:relative;overflow:hidden;display:flex;flex-direction:column;align-items:center;gap:12px;width:400px;max-width:calc(100vw - 48px);padding:40px 36px 36px;text-align:center;
background:var(--card);border:1px solid var(--border);border-radius:16px;box-shadow:0 1px 2px rgba(31,30,29,.04),0 12px 32px rgba(31,30,29,.06);animation:rise .35s __EASE__ both}
h1{margin:0;font-size:18px;font-weight:600;animation:fade-up .4s .15s ease-out both}
p{margin:0;font-size:14px;line-height:1.6;color:var(--muted);overflow-wrap:anywhere;animation:fade-up .4s .25s ease-out both}
p b{color:var(--fg);font-weight:600}
#hint{font-size:13px;animation-delay:.35s}
.mark{position:relative;width:84px;height:84px;display:flex;align-items:center;justify-content:center;margin-bottom:4px}
.tile{position:relative;width:64px;height:64px;border-radius:16px;border:1px solid var(--border);background:var(--tile);display:flex;align-items:center;justify-content:center;
font-size:26px;font-weight:600;line-height:1;animation:drop .55s __SPRING__ both}
.badge{position:absolute;right:-8px;bottom:-8px;width:26px;height:26px;border-radius:999px;background:var(--ok);border:3px solid var(--card);display:flex;align-items:center;justify-content:center;animation:pop .5s .35s __SPRING__ both}
.badge svg{width:14px;height:14px;fill:none;stroke:#fff;stroke-width:3.5;stroke-linecap:round;stroke-linejoin:round}
.badge path{stroke-dasharray:24;stroke-dashoffset:24;animation:draw .3s .55s ease-out forwards}
.ripple{position:absolute;right:-8px;bottom:-8px;width:26px;height:26px;border-radius:999px;border:2px solid var(--ok);opacity:0;animation:ring .9s .45s ease-out both}
.icon{width:56px;height:56px;border-radius:999px;display:flex;align-items:center;justify-content:center;margin-bottom:4px;background:var(--bad-soft);animation:pop .45s __EASE__ both}
.icon svg{width:28px;height:28px;fill:none;stroke:var(--bad);stroke-width:2.5;stroke-linecap:round;stroke-linejoin:round}
.bar{position:absolute;left:0;bottom:0;height:3px;width:100vw;max-width:400px;background:var(--ok);transform-origin:left;transform:scaleX(0)}
.closing .bar{animation:drain __SECONDS__s linear forwards}
@keyframes rise{from{opacity:0;transform:translateY(8px) scale(.98)}to{opacity:1;transform:none}}
@keyframes fade-up{from{opacity:0;transform:translateY(6px)}to{opacity:1;transform:none}}
@keyframes drop{from{opacity:0;transform:translateY(-14px) scale(.7)}to{opacity:1;transform:none}}
@keyframes pop{from{opacity:0;transform:scale(0)}to{opacity:1;transform:scale(1)}}
@keyframes ring{from{opacity:.7;transform:scale(.6)}to{opacity:0;transform:scale(2.4)}}
@keyframes draw{to{stroke-dashoffset:0}}
@keyframes drain{from{transform:scaleX(1)}to{transform:scaleX(0)}}
@media (prefers-reduced-motion:reduce){.card,h1,p,.tile,.badge,.badge path,.ripple,.icon,.closing .bar{animation:none}.badge path{stroke-dashoffset:0}.ripple{display:none}}
</style>
</head>
<body>
<div class="card">
__MARK__
<h1>__TITLE__</h1>
<p>__MESSAGE__</p>
<p id="hint">__HINT__</p>
__BAR__
</div>
__SCRIPT__
</body>
</html>
"""
# 先倒数，再试着关；浏览器不让关（不是脚本打开的窗口），就收起进度条、请人手动关。
COUNTDOWN = (
    "(function(){var n=__SECONDS__,el=document.getElementById('hint');"
    "function say(s){el.textContent='这个页面将在 '+s+' 秒后关闭。'}"
    "say(n);document.body.classList.add('closing');"
    "var t=setInterval(function(){n-=1;if(n>0){say(n);return}clearInterval(t);window.close();"
    "setTimeout(function(){document.body.classList.remove('closing');el.textContent='这个页面可以关掉。'},300)},1000)})();"
)


def callback_page(user=None, error=None, nonce=""):
    if user is not None:
        name = str(user.get("name") or "你")
        parts = {
            "__MARK__": '<div class="mark"><div class="tile">%s<div class="ripple"></div>'
            '<div class="badge"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg></div></div></div>'
            % html.escape(name.strip()[:1].upper()),
            "__TITLE__": "飞书已连接",
            "__MESSAGE__": "Sophon 现在可以用 <b>%s</b> 的身份发消息了。" % html.escape(name),
            "__HINT__": "这个页面可以关掉。",
            "__BAR__": '<div class="bar"></div>',
            "__SCRIPT__": '<script nonce="%s">%s</script>' % (html.escape(nonce, quote=True), COUNTDOWN),
        }
    else:
        # 失败的原因留着给人看完，不倒数、不自己关。
        parts = {
            "__MARK__": '<div class="icon"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg></div>',
            "__TITLE__": "授权没有完成",
            "__MESSAGE__": "飞书：%s" % html.escape(str(error)),
            "__HINT__": "关掉这个页面，回到 Sophon 再授权一次。",
            "__BAR__": "",
            "__SCRIPT__": "",
        }
    text = PAGE
    for key, value in parts.items():
        text = text.replace(key, value)
    return text.replace("__SECONDS__", str(CLOSE_SECONDS)).replace("__SPRING__", SPRING).replace("__EASE__", EASE)


def login(args):
    # 参数通常由插件带来；单独使用这个脚本时也可以在这里给。
    stored = load("config.json") or {}
    rest = list(args)
    browser = True
    while rest:
        flag = rest.pop(0)
        if flag == "--no-browser":
            browser = False
        elif flag == "--lark":
            stored["domain"] = "lark"
        elif flag in ("--app-id", "--app-secret") and rest:
            stored[flag[2:].replace("-", "_")] = rest.pop(0)
        else:
            raise Fail("用法：login [--app-id ID] [--app-secret SECRET] [--lark] [--no-browser]")
    if stored:
        save("config.json", stored)
    config = settings(complete=False)
    if not config.get("app_id"):
        raise Fail("还没有飞书应用的 App ID：在 Sophon 的「集成」页里填写")
    if not config.get("app_secret") and sys.stdin.isatty():
        # 不写在命令行里，免得留在 shell 历史。
        import getpass

        stored["app_secret"] = config["app_secret"] = getpass.getpass("App Secret: ").strip()
        save("config.json", stored)
    if not config.get("app_secret"):
        raise Fail("还没有飞书应用的 App Secret：在 Sophon 的「集成」页里填写")

    state = secrets.token_urlsafe(16)
    result = {}

    class Callback(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            url = urllib.parse.urlparse(self.path)
            query = urllib.parse.parse_qs(url.query)
            if url.path != "/feishu/oauth":
                self.send_response(404)
                self.end_headers()
                return
            nonce = secrets.token_urlsafe(16)
            try:
                if query.get("state", [""])[0] != state or not query.get("code"):
                    raise Fail(query.get("error", ["授权没有完成"])[0])
                user = exchange(config, {"grant_type": "authorization_code", "code": query["code"][0], "redirect_uri": REDIRECT})
                result["user"] = user
                text = callback_page(user=user, nonce=nonce)
            except Fail as error:
                result["error"] = str(error)
                text = callback_page(error=error)
            self.send_response(200 if "user" in result else 400)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            # 这一页只许跑它自己那段倒数脚本，别的一概不许。
            self.send_header(
                "Content-Security-Policy",
                "default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-%s'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" % nonce,
            )
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(text.encode("utf-8"))

    try:
        server = HTTPServer(("127.0.0.1", PORT), Callback)
    except OSError as error:
        raise Fail("本机的 %d 端口被占用（%s）：关掉占用它的程序再试" % (PORT, error))
    query = urllib.parse.urlencode(
        {"client_id": config["app_id"], "redirect_uri": REDIRECT, "scope": " ".join(SCOPES), "state": state, "response_type": "code"}
    )
    address = "%s/open-apis/authen/v1/authorize?%s" % (bases(config)[1], query)
    print("在浏览器里同意授权：\n%s" % address, flush=True)
    if browser:
        subprocess.call(["open", address], stderr=subprocess.DEVNULL)
    server.timeout = 1
    deadline = time.time() + 300
    while not result and time.time() < deadline:
        server.handle_request()
    server.server_close()
    if "user" not in result:
        raise Fail(result.get("error") or "等了五分钟没有等到授权")
    print("已授权：%s" % result["user"]["name"])


# ---------------------------------------------------------------- send


def send(args):
    """给一个人或一个群发一条文字消息，以你的身份。"""
    rest = list(args)
    to, text = None, None
    kinds = {"--email": "email", "--open-id": "open_id", "--chat": "chat_id"}
    while rest:
        flag = rest.pop(0)
        if flag in kinds and rest:
            to = (kinds[flag], rest.pop(0).strip())
        elif flag == "--text" and rest:
            text = rest.pop(0)
        else:
            raise Fail("用法：send (--email 邮箱 | --open-id ID | --chat 群ID) --text 文字")
    if not to or not to[1] or not (text or "").strip():
        raise Fail("用法：send (--email 邮箱 | --open-id ID | --chat 群ID) --text 文字")
    token = signed_in()["access_token"]
    body = {"receive_id": to[1], "msg_type": "text", "content": json.dumps({"text": text}, ensure_ascii=False)}
    api("POST", "/open-apis/im/v1/messages", token, body, {"receive_id_type": to[0]})
    print("已发送")


# ---------------------------------------------------------------- find


def find(args):
    """按名字找同事，以你的身份：你在飞书里搜得到谁，这里就找得到谁。"""
    query = " ".join(args).strip()
    if not query:
        raise Fail("用法：find 名字")
    token = signed_in()["access_token"]
    try:
        found = api("GET", "/open-apis/search/v1/user", token, None, {"query": query, "page_size": "10"})
    except Fail as error:
        # 99991679：这次授权里没有这项权限；99991672：应用还没开通它。
        if "99991679" in str(error) or "99991672" in str(error):
            raise Fail(
                "还不能在飞书里找人：先在飞书开放平台给应用开通用户身份权限「搜索用户」（contact:user:search），"
                "再到 Sophon 的「集成」页里重新授权一次"
            )
        raise
    users = [user for user in found.get("users") or [] if user.get("open_id")]
    if not users:
        print("飞书里没有找到「%s」" % query, file=sys.stderr)
    for user in users:
        print(json.dumps({"name": user.get("name"), "open_id": user["open_id"]}, ensure_ascii=False))


def status(_args):
    user = load("user.json") or {}
    usable = user.get("expires_at", 0) > time.time() or user.get("refresh_token")
    if not user.get("open_id") or not usable:
        raise Fail("还没有授权")
    print("已授权：%s" % user.get("name"))


def main(argv):
    commands = {"login": login, "status": status, "send": send, "find": find}
    if len(argv) < 2 or argv[1] not in commands:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    try:
        commands[argv[1]](argv[2:])
    except Fail as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
