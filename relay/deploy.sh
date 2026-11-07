#!/bin/sh
# Installs (or updates) sophon-relay on a Linux server over SSH and starts
# it on port 8443:  relay/deploy.sh root@1.2.3.4 [-i key.pem]
# Open TCP 8443 in the server's firewall, then set the relay in Sophon:
# Settings → Relay server → 1.2.3.4:8443.
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
