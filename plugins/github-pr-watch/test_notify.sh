#!/bin/sh
# notify 的通知和配对，对着假的 gh 和假的 sophon 跑：sh test_notify.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/bin"

# 假的 gh：提交记录里的邮箱是 $FAKE_COMMIT_EMAIL。
cat >"$work/bin/gh" <<'SH'
#!/bin/sh
printf '%s\n' "${FAKE_COMMIT_EMAIL:-}"
SH
# 假的 sophon：记下每次调用；飞书只认 ada@x.com 和 ou_ 开头的；plugin set 把值存下。
cat >"$work/bin/sophon" <<'SH'
#!/bin/sh
printf '%s\n' "$*" | head -1 >>"$FAKE_LOG"
case "$1 $2" in
"use feishu")
    case "$5" in ada@x.com | ou_*) echo 已发送 ;; *) echo "飞书 /open-apis/im/v1/messages：230001 invalid receive_id" >&2; exit 1 ;; esac
    ;;
"plugin set") printf '%s' "${4#people=}" >"$FAKE_PEOPLE" ;;
esac
SH
chmod +x "$work/bin/gh" "$work/bin/sophon"

export PATH="$work/bin:/usr/bin:/bin" FAKE_LOG="$work/log" FAKE_PEOPLE="$work/people" SOPHON_PLUGIN_DATA="$work/plugin-data/github-pr-watch"
export SOPHON_ACTION=reply SOPHON_TEXT="已 approve" SOPHON_VAR_AUTHOR=Uno-FN SOPHON_VAR_REPO=acme/app SOPHON_VAR_NUMBER=12 SOPHON_VAR_URL=https://github.com/acme/app/pull/12
failed=0
check() { # 说明 期望 实际
    if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: 期望「$2」，得到「$3」"; failed=1; fi
}
last() { tail -1 "$FAKE_LOG" 2>/dev/null | cut -d' ' -f1-5; }

# 没写对应关系：用提交记录里的邮箱。
FAKE_COMMIT_EMAIL=ada@x.com SOPHON_PARAM_PEOPLE="" "$here/notify" reply >/dev/null
check "提交记录里的邮箱飞书认得就发" "use feishu send --email ada@x.com" "$(last)"

# 飞书不认这个邮箱：说不知道是谁，让 AI 去配对。
said=$(FAKE_COMMIT_EMAIL=uno@corp.com SOPHON_PARAM_PEOPLE="" "$here/notify" reply 2>&1) && code=0 || code=$?
check "飞书不认的邮箱算失败" 1 "$code"
case "$said" in *"不知道 @Uno-FN 在飞书里是谁"*"先配对"*) ok=yes ;; *) ok=$said ;; esac
check "并说清要先配对" yes "$ok"

# 没有邮箱可试，也一样。
said=$(FAKE_COMMIT_EMAIL="" SOPHON_PARAM_PEOPLE="" "$here/notify" reply 2>&1) && code=0 || code=$?
case "$said" in *"不知道 @Uno-FN 在飞书里是谁"*) ok=yes ;; *) ok=$said ;; esac
check "没有邮箱可试也是不知道是谁" "1 yes" "$code $ok"

# 配对：记在对应关系里，别人的留着，同一个账号的旧的换掉。
SOPHON_PARAM_PEOPLE="bob=bob@x.com, uno-fn=old@x.com" "$here/notify" use pair Uno-FN ou_uno "吴诺 Uno" >/dev/null
check "配对写进对应关系" "bob=bob@x.com,uno-fn=ou_uno:吴诺 Uno" "$(cat "$FAKE_PEOPLE")"
check "写的是这个插件自己的设置" "plugin set github-pr-watch" "$(tail -1 "$FAKE_LOG" | cut -d' ' -f1-3)"

# 记下之后直接按 open_id 发，不再看提交记录。
FAKE_COMMIT_EMAIL=uno@corp.com SOPHON_PARAM_PEOPLE="$(cat "$FAKE_PEOPLE")" "$here/notify" reply >/dev/null
check "配对过的按 open_id 发" "use feishu send --open-id ou_uno" "$(last)"

# 你自己写的邮箱照旧。
SOPHON_PARAM_PEOPLE="uno-fn=ada@x.com" "$here/notify" reply >/dev/null
check "写了邮箱的按邮箱发" "use feishu send --email ada@x.com" "$(last)"

# 乱写的 open_id 不收。
"$here/notify" use pair uno-fn "not an id" >/dev/null 2>&1 && code=0 || code=$?
check "不是 open_id 的不记" 2 "$code"

# 关了通知，或不是在说结论：什么也不发。
before=$(wc -l <"$FAKE_LOG")
SOPHON_PARAM_NOTIFY=no "$here/notify" reply
SOPHON_ACTION=done "$here/notify" reply
check "关了通知或不是结论时不发" "$before" "$(wc -l <"$FAKE_LOG")"

exit "$failed"
