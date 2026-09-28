# Security Policy

KaspaBattle is experimental software running on Kaspa **testnet only** and is under active development. We still take security reports seriously — testnet issues in escrow, payout, auth or admin logic often point at real design flaws worth fixing before any future mainnet work.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Instead, use one of:

1. [GitHub Security Advisories](https://github.com/TimBee706/KaspaBattle2/security/advisories/new) (private, preferred)
2. Email the address listed on [kaspabattle.com/support](https://www.kaspabattle.com/support), if you can't use GitHub Advisories

Please include:
- A description of the issue and its potential impact
- Steps to reproduce (testnet match/tournament/lobby IDs are fine — never send a mnemonic or private key)
- Any relevant logs or transaction IDs

## Scope

In scope: the `battle-frontend` and `kaspabattle` (Rust backend, escrow/multisig/payout logic) code in this repository.

Out of scope: third-party services this project depends on (FACEIT, the Kaspa network itself, hosting infrastructure) — please report those directly to their respective maintainers.
