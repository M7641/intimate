# Phase 0 — Guardrails

**When:** Week 1, before provisioning anything else.
**Time:** ~45 minutes, plus a domain purchase.

## Goal

Make the two expensive mistakes impossible before there's anything to make them with.
Runaway bills and compromised root accounts are the most common way this goes wrong
for self-directed learners, and both are trivially prevented in advance.

> **A note on the console paths below:** AWS reorganises its console regularly, so
> exact menu wording may drift. The service names are stable — if a path doesn't
> match, use the search box at the top of the console for the service name in
> **bold** and you'll land in the right place.

---

## 1. Create the AWS account

Go to <https://portal.aws.amazon.com/billing/signup>.

You'll need an email address that isn't already tied to an AWS account, and a
credit/debit card. AWS places a small temporary authorisation (~£1) to verify it,
which is refunded.

Pick **Personal** for account type and **Basic support — Free** for the support plan.

The email and password you set here are the **root user**. This is the most
privileged thing in the account and cannot be restricted by any policy. Treat it
accordingly.

---

## 2. MFA on the root user

Do this immediately. An unprotected root account is the highest-severity problem you
can have on AWS.

### Choose your second factor first

| Option | Notes |
|---|---|
| **Authenticator app (TOTP)** | Easiest. On Linux: Aegis or Ente Auth on your phone, or a password manager with TOTP support (Bitwarden, 1Password). |
| **Hardware security key** | Strongest, phishing-resistant. A YubiKey is ~£45. Worth it eventually; not required today. |

Storing the TOTP seed in the same password manager as the password does reduce the
"second factor" to "one factor with extra steps". For a solo learner that's an
acceptable trade — just be aware you're making it.

### The steps

1. Sign in at <https://console.aws.amazon.com> — choose **Root user**, enter the
   account email and password.
2. Click your **account name (top right)** → **Security credentials**.
3. Find the **Multi-factor authentication (MFA)** panel → **Assign MFA device**.
4. Give it a device name (e.g. `phone-aegis`), pick **Authenticator app**, → **Next**.
5. Click **Show QR code**, scan it with your app.
6. Enter **two consecutive codes** from the app — wait for the first to roll over
   before entering the second. This trips people up; it wants code N and code N+1,
   not the same code twice.
7. **Add MFA**.

### Then do it again

AWS supports **up to 8 MFA devices** on the root user, and provides **no backup
codes**. If you lose your only device, recovery means an email + phone verification
dance with AWS support that can take days.

So register a second device now. A second authenticator app on a different device, or
a hardware key kept in a drawer. This is five minutes that saves a genuinely terrible
week.

### Verify

Sign out, sign back in as root. It should demand a code.

---

## 3. Create an IAM user for daily use

### Why not just use root?

Root cannot be restricted, cannot be scoped by policy, and its credentials appear in
every credential-stuffing list eventually. Every real AWS setup uses root for account
management only and does day-to-day work under a lesser identity. Building that habit
now costs nothing.

> **The modern alternative:** AWS now recommends **IAM Identity Center** (formerly SSO)
> over IAM users, because it issues short-lived credentials instead of permanent keys.
> It's better practice and genuinely worth learning — but it's more setup, and for one
> person on one account an IAM user is the simpler starting point. Revisit this when
> you have a second AWS account, which is the point where Identity Center starts
> paying for itself.

### The steps

Still signed in as root:

1. Console search box → **IAM** → **Users** (left nav) → **Create user**.
2. **User name:** `mike`.
3. Tick **Provide user access to the AWS Management Console**.
4. Choose **I want to create an IAM user** (not Identity Center, per the note above).
5. **Custom password** → set a strong one. Untick *"Users must create a new password
   at next sign-in"* — you're both users here.
6. **Next** → Permissions options → **Attach policies directly**.
7. Tick **AdministratorAccess**.
8. **Next** → **Create user**.
9. On the confirmation screen, **copy the console sign-in URL** —
   `https://<account-id>.signin.aws.amazon.com/console`. Save it in your password
   manager alongside the credentials.

`AdministratorAccess` is broad — this user can do nearly everything root can. The
protection it buys is that it *can* be revoked, scoped or deleted from root, and it
can be given its own MFA. That's a real difference, not a cosmetic one.

### Nicer sign-in URL (optional, 1 minute)

**IAM** → **Dashboard** → **Account Alias** panel → **Create** → pick something like
`mike-lab`. Your sign-in URL becomes
`https://mike-lab.signin.aws.amazon.com/console`, which is far easier to remember
than a 12-digit account number.

### MFA on the IAM user too

1. **IAM** → **Users** → click `mike` → **Security credentials** tab.
2. **Multi-factor authentication (MFA)** → **Assign MFA device**.
3. Same flow as before — name it, authenticator app, scan, two consecutive codes.

### Verify, then retire root

1. Sign out.
2. Sign in at your account alias URL as `mike`, with MFA.
3. Confirm you can reach the IAM and EC2 consoles.

**From here, root is only for:** billing settings, closing the account, and the one
billing-access toggle in the next step. Nothing else.

---

## 4. Billing alarms

### The gotcha to know about first

By default, **IAM users cannot see billing information, even with
`AdministratorAccess`.** Billing is gated behind a separate account-level switch that
only root can flip. If you skip this, the next section will 403 at you and it won't
be obvious why.

**As root:**

1. **Account name (top right)** → **Account**.
2. Scroll to **IAM user and role access to Billing information** → **Edit**.
3. Tick **Activate IAM Access** → **Update**.

Now sign back in as `mike` for the rest.

### Turn on the underlying billing metric

1. Console search → **Billing and Cost Management**.
2. **Billing preferences** (left nav).
3. Under **Alert preferences** → **Edit**:
   - Tick **Receive AWS Free Tier alerts** (warns as you approach free-tier limits)
   - Tick **Receive CloudWatch billing alerts**
   - Add your email address
4. **Update**.

The CloudWatch one matters: without it, the billing metric that alarms read from
isn't published at all. It can take up to 24 hours to start producing data.

### Create the budgets

1. **Billing and Cost Management** → **Budgets** (left nav) → **Create budget**.
2. Choose **Customize (advanced)** → **Cost budget** → **Next**.
3. **Budget name:** `learning-early-warning`
4. **Period:** Monthly. **Budget renewal type:** Recurring.
5. **Budgeting method:** Fixed.
6. **Enter your budgeted amount:** `10`
   > Budgets use your account's billing currency, which is likely USD unless you set
   > GBP at signup. Check which you have and adjust the number — the exact figure
   > matters less than it being far below what you'd tolerate.
7. **Next** → **Add an alert threshold**:
   - Threshold: **80%** of **Actual** cost → your email
   - Add a second: **100%** of **Forecasted** cost → your email
8. **Next** → **Create budget**.

Then repeat the whole thing for a `learning-backstop` budget at **30**, alerting at
100% actual.

> **Cost of the budgets themselves:** the first **two** budgets are free; further ones
> cost about $0.02/day. Two is exactly what's specified here — don't casually add a
> third.

### Why £10 when the budget is £20-50?

The alarm is deliberately far below the real ceiling. The point is to hear about a
mistake within a day of making it, not at the end of the month when it's £200. Expect
the £10 alert to fire during normal phase-3 work — that's it functioning correctly,
not a false alarm.

### Verify

Budgets take up to 24h to first evaluate. Come back tomorrow and confirm the budget
page shows an actual spend figure rather than "—".

---

## 5. AWS CLI on your laptop

You'll want this from phase 3 onward, and it's the fastest way to verify the steps
above.

```
sudo pacman -S aws-cli-v2
aws --version
```

### Credentials

**IAM** → **Users** → `mike` → **Security credentials** tab → **Access keys** →
**Create access key** → choose **Command Line Interface (CLI)** → acknowledge the
warning → **Create access key**.

Copy both values — the secret is shown **once and never again**.

```
aws configure
```

- **AWS Access Key ID:** paste
- **AWS Secret Access Key:** paste
- **Default region name:** `eu-west-2` (London — closest, and keeps data in the UK)
- **Default output format:** `json`

This writes `~/.aws/credentials`. Check it's not world-readable:

```
chmod 600 ~/.aws/credentials
ls -l ~/.aws/credentials
```

> **This does not contradict the "never paste access keys" rule in
> [04-aws-terraform.md](04-aws-terraform.md).** That rule is about keys on *servers*
> and in *deployed config*, where an IAM role should supply credentials automatically
> and a leaked key is a breach. A key in `~/.aws/credentials` on your own laptop is
> the normal way a human uses the CLI. Never commit it, never put it in a Dockerfile,
> and rotate it if it ever leaves the machine.

### Verify everything at once

```
# Who am I? Should show the mike IAM user, not root.
aws sts get-caller-identity

# AccountMFAEnabled should be 1 -- this is root's MFA status.
aws iam get-account-summary --query 'SummaryMap.AccountMFAEnabled'

# Should list the MFA device you attached to the IAM user.
aws iam list-mfa-devices --user-name mike

# Should list your two budgets.
aws budgets describe-budgets --account-id "$(aws sts get-caller-identity --query Account --output text)" --query 'Budgets[].BudgetName'
```

If all four behave, phase 0's AWS half is genuinely done — not just clicked through.

---

## 6. Hetzner account

1. Sign up at <https://accounts.hetzner.com/signUp>. Expect an identity check on a
   new account; it can take a few hours, which is why this is worth doing in week 1
   rather than on the evening you want to build something.
2. Go to <https://console.hetzner.cloud> → **New project** → name it `lab`.

### SSH key

If you don't already have a key:

```
ssh-keygen -t ed25519 -C "mike@$(hostname)"
cat ~/.ssh/id_ed25519.pub
```

Ed25519 over RSA — shorter, faster, and the current default recommendation.

In the Hetzner console: **Security** (left nav) → **SSH keys** → **Add SSH key** →
paste the **`.pub`** contents → name it `laptop`.

Adding it here means every server you create can have it installed at build time, so
you never need password SSH at all — which is what makes the hardening in phase 1
straightforward.

**No server yet.** That's [phase 1](02-first-server.md).

---

## 7. A domain

Buy one. ~£10/year, and it's the single best-value item in the whole plan.

Real DNS and real TLS behave differently from `localhost` and `nip.io` tricks —
certificate issuance, propagation delays, apex vs. subdomain records, caching. Every
later phase is better with a real domain in it.

**Registrar:** barely matters. **Cloudflare Registrar** and **Porkbun** both sell at
cost with no renewal-price bait-and-switch. Avoid GoDaddy, whose cheap first year
becomes an expensive habit.

**What to avoid:** a `.io` or `.ai` vanity TLD at £40+/year. A `.com`, `.net`, or
`.uk` is fine, and £10 vs £40 matters when it's a learning expense.

Nothing to configure yet — you'll point records at a server in phase 1. Just own it.

---

## Done when

- [ ] Signing in as AWS root demands a second factor
- [ ] **Two** MFA devices are registered on root
- [ ] Daily work happens as the `mike` IAM user, which also has MFA
- [ ] `aws sts get-caller-identity` shows the IAM user
- [ ] IAM access to billing is activated
- [ ] Two budgets exist, and `aws budgets describe-budgets` lists them
- [ ] You've received at least one email from AWS confirming alert setup
- [ ] Hetzner account verified, project created, SSH key uploaded
- [ ] You own a domain and can edit its DNS records

## Notes

Write anything learned here — especially anything where the console didn't match
these instructions, so future-you knows the doc drifted.
