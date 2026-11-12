#!/bin/sh
# Installs (or updates) sophon-relay on a Linux server over SSH and starts
# it on port 8443:  relay/deploy.sh root@1.2.3.4 [-i key.pem]
# Open TCP 8443 in the server's firewall, then set the relay in Sophon:
# Settings → Relay server → 1.2.3.4:8443.
#
# The same service is the push gateway (/push, docs/push.md): it sends the
# computer's notifications to Apple for the paired iPhones. For that, give
# the server the app's APNs key once (Apple Developer → Keys: the .p8 file,
# its key id, and the team id); this script does not copy it:
#   scp AuthKey_ABC123DEFG.p8 root@1.2.3.4:/etc/sophon-relay-apns.p8
#   ssh root@1.2.3.4 'chmod 600 /etc/sophon-relay-apns.p8
#     printf "APNS_KEY_ID=ABC123DEFG\nAPNS_TEAM_ID=TEAM123456\n" > /etc/sophon-relay.env
#     systemctl restart sophon-relay'
# (APNS_TOPIC=<bundle id> in that file too, when it is not dev.sophon.app.)
# Without them the relay works as before and pushes are answered
# NotConfigured. The service reads the .p8 as a systemd credential (see
# sophon-relay.service); run by hand, set APNS_KEY_FILE to its path.
# The gateway's own key is /var/lib/sophon-relay/push.key, made on the first
# start: keep it when moving the server, computers refuse a gateway whose
# key changed.
set -eu
cd "$(dirname "$0")"
[ $# -ge 1 ] || { echo "usage: $0 user@host [ssh options]" >&2; exit 1; }
HOST=$1; shift
[ -x dist/sophon-relay ] || ./build-linux.sh
scp "$@" dist/sophon-relay sophon-relay.service "$HOST":/tmp/
ssh "$@" "$HOST" 'set -e
  sudo install -m 755 /tmp/sophon-relay /usr/local/bin/sophon-relay
  sudo install -m 644 /tmp/sophon-relay.service /etc/systemd/system/sophon-relay.service
  sudo systemctl daemon-reload
  sudo systemctl enable --now sophon-relay
  sudo systemctl restart sophon-relay
  sleep 1; systemctl --no-pager --lines=3 status sophon-relay'
