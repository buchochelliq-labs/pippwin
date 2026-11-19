# Security policy

## Reporting a vulnerability

Please report security problems privately, through GitHub's private
vulnerability reporting: on
<https://github.com/buchochelliq-labs/pippwin/security>, choose
**Report a vulnerability**. Do not open a public issue for them.

Include what you found, how to reproduce it, and what an attacker could do
with it. You will get an answer as soon as a maintainer has looked at it,
and a fix is coordinated with you before anything is made public.

In scope: pippwin itself (`src/`), the bootstrap scripts (`scripts/`), and
how they handle downloads, the Apple login, the API key, xtool's login
token and a project's `pippwin.toml`. Problems in xtool, Swift, rustup,
Node.js or usbipd-win belong to those projects.

## Supported versions

Fixes go to `main`; build the latest `main` or the latest tag.

## How pippwin handles secrets

See <https://buchochelliq-labs.github.io/pippwin/security/>.
