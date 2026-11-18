#!/usr/bin/env bash
# Installs Ubuntu packages on a CI runner. An EC2 runner's regional archive mirror can list files it does not
# have yet (404 on the amd64v3 builds of 26.04 in ap-south-2); the install then retries against the main archive.
set -euo pipefail

sudo apt-get update
if ! sudo apt-get install -y "$@"; then
  for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
    [ -f "$f" ] && sudo sed -i -E 's#https?://[a-z0-9.-]+\.ec2\.archive\.ubuntu\.com/ubuntu#http://archive.ubuntu.com/ubuntu#g' "$f"
  done
  sudo apt-get update
  sudo apt-get install -y "$@"
fi
