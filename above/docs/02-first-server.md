# Phase 1 — One server, done properly

**When:** Weeks 1-4.
**Cost:** Hetzner CX22, ~€4/month.

## Goal

Close the "shaky on servers" gap. Take one box from bare Ubuntu to serving real
traffic over HTTPS on a real domain, doing every layer by hand, with nothing
abstracting it away.

Deploy anything — a static site, a small API. The app doesn't matter. The surface
around it is the lesson.

## Work

### 1. Provision

Hetzner CX22, Ubuntu LTS, with your SSH key attached at creation time.

### 2. Users and SSH

Do this first, before the box has been up long enough to be found by scanners.

```
adduser mike
usermod -aG sudo mike
rsync --archive --chown=mike:mike ~/.ssh /home/mike
```

Then in `/etc/ssh/sshd_config`:

```
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
```

```
systemctl reload ssh
```

**Before closing your current session, open a second terminal and confirm you can
still log in.** Locking yourself out of a box is a rite of passage best skipped.

### 3. Firewall

```
ufw default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw allow 80,443/tcp
ufw enable
```

`ufw` is a friendly front-end to `nftables`. Fine to start here; worth reading the
generated rules later with `nft list ruleset` to see what it actually did.

### 4. Automatic security updates

```
apt install unattended-upgrades
dpkg-reconfigure --priority=low unattended-upgrades
```

### 5. DNS

Point an A record at the server's IPv4, and an AAAA at its IPv6. Watch propagation
with `dig +short yourdomain.com`.

Understand *why* an A record and a CNAME aren't interchangeable, and why you can't
CNAME an apex domain — this comes up constantly later.

### 6. Reverse proxy and TLS — Caddy

Caddy is the recommendation over nginx here specifically because it obtains and
renews Let's Encrypt certificates automatically. One line of config gets working
HTTPS instead of an afternoon fighting certbot. Fight certbot later, deliberately,
when you want to understand ACME.

`/etc/caddy/Caddyfile`:

```
yourdomain.com {
    reverse_proxy localhost:8080
}
```

```
systemctl reload caddy
```

### 7. The app as a systemd unit

`/etc/systemd/system/myapp.service`:

```
[Unit]
Description=My app
After=network.target

[Service]
User=app
WorkingDirectory=/srv/myapp
ExecStart=/srv/myapp/bin/server
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```
systemctl daemon-reload
systemctl enable --now myapp
journalctl -u myapp -f
```

Running as a dedicated unprivileged `app` user, restarting on crash, starting on
boot, logging to the journal. This is the actual job systemd does, and understanding
it removes most of the mystery from "how does software run on a server."

## The exercise that matters

**Delete the server and rebuild it from nothing.**

1. Rebuild by hand. Time it. Note every step you'd forgotten.
2. Delete it and do it again, writing each step down as you go.
3. Delete it and do it a third time — as a shell script.

This will be tedious and that is entirely the point. You arrive at
infrastructure-as-code because you felt the problem it solves, not because a tutorial
told you to. People who skip this step end up writing Terraform they don't
understand.

## Worth seeing with your own eyes

After a week, read the logs:

```
journalctl -u ssh | grep -i "invalid user" | tail -50
```

The constant background of brute-force attempts and scanner traffic hitting a random
box on the internet is a formative thing to see directly rather than read about.

## Done when

- [ ] HTTPS works on your real domain with a valid certificate
- [ ] Password SSH is off and root login is disabled
- [ ] The app survives `reboot` without you touching it
- [ ] The app restarts by itself after `kill -9`
- [ ] You have rebuilt the whole box from scratch at least twice
- [ ] A script exists that does the rebuild
- [ ] You can explain what's in your firewall rules and why

## Notes
