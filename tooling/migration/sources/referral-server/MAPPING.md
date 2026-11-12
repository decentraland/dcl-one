# referral-server → catalyst

Pinned at `828c30c215e7ce3a2f1d8ea57c202f4a21ebfa0e` (two migrations). The
service is superseded by social-service-ea, which carries the same tables
plus `referral_emails`; this source runs after it and only fills gaps.

| Upstream | Target | Notes |
| --- | --- | --- |
| `referral_progress` | `social.referral_progress` | Earliest row per `lower(invited_user)` (as social-service-ea `1782864000000` dedupes); `tier_granted = true` → status `tier_granted`. Updates an existing row only for the same referrer and only to a later status; `rejected_ip_match` stays. Signup/tier timestamps and the granted flag are preserved; equal-status rows can fill missing fields. Source row IDs are replaced by target IDs. The source has no IP column, so target IPs are preserved. Watermark `updated_at`. |
| `referral_progress_viewed` | `social.referral_progress_viewed` | The viewed count only grows. Watermark `updated_at`. |

Skipped: self-referrals (the target forbids them), wallets that are not
`0x` + 40 hex, statuses outside `pending`/`signed_up`/`tier_granted`.
`social.referral_emails` is not here: this service never had it.

Restart `referral_progress` after upgrading an earlier import to fill unchanged historical rows. Target reward execution fields and login days are untouched.
