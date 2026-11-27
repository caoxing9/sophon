#!/bin/sh
# Locks down the machine a relay server (deploy.sh, p2p-setup.sh) runs on:
#
#   relay/harden.sh root@1.2.3.4 [ssh options]
#
# - SSH by key alone: no passwords, so guessing them gets nowhere. Check
#   that your key logs in first; the cloud's web console stays a way in.
# - Security updates on their own (unattended-upgrades), when not already.
#
# Run again after changing it: each part replaces its last.
set -eu
[ $# -ge 1 ] || { echo "usage: $0 user@host [ssh options]" >&2; exit 1; }
HOST=$1; shift
ssh "$@" "$HOST" 'sh -se' <<'HARDEN'
  set -eu
  # SSH: keys only (this file comes before cloud-init's, so it wins).
  sudo tee /etc/ssh/sshd_config.d/10-sophon-keys-only.conf >/dev/null <<SSHD
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin prohibit-password
MaxAuthTries 3
SSHD
  sudo sshd -t
  sudo systemctl reload ssh

  # Security updates, daily.
  if ! dpkg -s unattended-upgrades >/dev/null 2>&1; then sudo apt-get install -y unattended-upgrades; fi
  printf 'APT::Periodic::Update-Package-Lists "1";\nAPT::Periodic::Unattended-Upgrade "1";\n' | sudo tee /etc/apt/apt.conf.d/20auto-upgrades >/dev/null

  sshd -T | grep -Ei '^(passwordauthentication|permitrootlogin) '
HARDEN
