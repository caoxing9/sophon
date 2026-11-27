#!/bin/sh
# Turns on peer to peer's relay (iroh's) in sophon-relay on a server that
# runs it already (deploy.sh), with a Let's Encrypt certificate for the
# server's IP address: no domain, no ICP filing.
#
#   relay/p2p-setup.sh root@1.2.3.4 1.2.3.4 [ssh options]
#
# Open these in the server's firewall (the cloud's security group):
#   TCP 80    Let's Encrypt checks the address here (while renewing)
#   TCP 3443  the relay (HTTPS)
#   UDP 7842  address discovery: how computers and phones learn their own
#             public addresses, so they can punch through to each other
# (TCP 3340, the captive portal probe, helps too.)
#
# The certificate lives six days (Let's Encrypt's for IP addresses): a timer
# renews it daily when due, with lego, and hands it to the service, which
# reads it again every hour. No restart.
set -eu
cd "$(dirname "$0")"
[ $# -ge 2 ] || { echo "usage: $0 user@host <public IPv4> [ssh options]" >&2; exit 1; }
HOST=$1; IP=$2; shift 2
case "$IP" in *[!0-9.]*|'') echo "Not an IPv4 address: $IP" >&2; exit 1 ;; esac
LEGO_VERSION=v5.7.0
mkdir -p dist
if [ ! -x dist/lego ]; then
  # From here, not the server: GitHub is slow or unreachable from some.
  tmp=$(mktemp -d)
  curl -fsSL -o "$tmp/lego.tar.gz" "https://github.com/go-acme/lego/releases/download/$LEGO_VERSION/lego_${LEGO_VERSION}_linux_amd64.tar.gz"
  tar -xzf "$tmp/lego.tar.gz" -C "$tmp" lego
  install -m 755 "$tmp/lego" dist/lego
  rm -rf "$tmp"
fi
scp "$@" dist/lego "$HOST:/tmp/sophon-relay-lego"
ssh "$@" "$HOST" "IP='$IP' sh -se" <<'SETUP'
  set -eu
  sudo install -m 755 /tmp/sophon-relay-lego /usr/local/bin/lego && rm -f /tmp/sophon-relay-lego
  # Hands a new certificate to the service: into its own state directory
  # (it runs as a user of its own, which cannot read root's files).
  sudo tee /usr/local/sbin/sophon-relay-cert-deploy >/dev/null <<HOOK
#!/bin/sh
set -eu
dir=/var/lib/sophon-relay/tls
owner=\$(stat -c %U:%G /var/lib/sophon-relay/.)
install -d -m 700 -o "\${owner%:*}" -g "\${owner#*:}" "\$dir"
install -m 644 -o "\${owner%:*}" -g "\${owner#*:}" /etc/sophon-relay/lego/certificates/$IP.crt "\$dir/cert.pem"
install -m 600 -o "\${owner%:*}" -g "\${owner#*:}" /etc/sophon-relay/lego/certificates/$IP.key "\$dir/key.pem"
HOOK
  sudo chmod 755 /usr/local/sbin/sophon-relay-cert-deploy
  # Gets the certificate, or renews it when due; then hands it over.
  sudo tee /usr/local/sbin/sophon-relay-cert >/dev/null <<CERT
#!/bin/sh
exec /usr/local/bin/lego run --accept-tos --path /etc/sophon-relay/lego --domains $IP --http --profile shortlived --deploy-hook /usr/local/sbin/sophon-relay-cert-deploy "\$@"
CERT
  sudo chmod 755 /usr/local/sbin/sophon-relay-cert
  sudo install -d -m 700 /etc/sophon-relay
  sudo /usr/local/sbin/sophon-relay-cert --no-random-sleep
  sudo /usr/local/sbin/sophon-relay-cert-deploy
  sudo tee /etc/systemd/system/sophon-relay-cert.service >/dev/null <<UNIT
[Unit]
Description=Renew sophon-relay's peer to peer certificate when due
After=network-online.target
[Service]
Type=oneshot
ExecStart=/usr/local/sbin/sophon-relay-cert
UNIT
  sudo tee /etc/systemd/system/sophon-relay-cert.timer >/dev/null <<UNIT
[Unit]
Description=Renew sophon-relay's peer to peer certificate when due
[Timer]
OnCalendar=*-*-* 04:00:00
RandomizedDelaySec=1h
Persistent=true
[Install]
WantedBy=timers.target
UNIT
  # The service's settings for peer to peer, kept beside the others.
  touch /etc/sophon-relay.env && chmod 600 /etc/sophon-relay.env
  sed -i '/^P2P_/d' /etc/sophon-relay.env
  printf 'P2P_RELAY_URL=https://%s:3443\nP2P_TLS_CERT=/var/lib/sophon-relay/tls/cert.pem\nP2P_TLS_KEY=/var/lib/sophon-relay/tls/key.pem\n' "$IP" >> /etc/sophon-relay.env
  sudo systemctl daemon-reload
  sudo systemctl enable --now sophon-relay-cert.timer
  sudo systemctl restart sophon-relay
  sleep 2; journalctl -u sophon-relay --no-pager -n 5 | grep -i "p2p relay" || true
SETUP
