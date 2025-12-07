# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Which versions are eligible for receiving such patches depends on the CVSS v3.0 Rating:

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |
| < Latest | :x:                |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

1. **GitHub Security Advisories** (preferred):
   - Go to https://github.com/yourusername/lpm/security/advisories/new
   - Click "New draft security advisory"
   - Fill out the form with details about the vulnerability

2. **Email** (if GitHub is not accessible):
   - Please use GitHub Security Advisories if possible
   - If email is necessary, contact the repository maintainers
   - Subject: "Security Vulnerability in LPM"
   - Include as much detail as possible about the vulnerability

## What to Include

When reporting a vulnerability, please include:

- Type of vulnerability (e.g., XSS, injection, etc.)
- Full paths of source file(s) related to the vulnerability
- Location of the affected code (tag/branch/commit or direct URL)
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the vulnerability

## Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Resolution**: Depends on severity and complexity

## Disclosure Policy

- We will acknowledge receipt of your vulnerability report within 48 hours
- We will provide an initial assessment within 7 days
- We will keep you informed of our progress
- We will notify you when the vulnerability is fixed
- We will credit you in the security advisory (if you wish)

## Security Best Practices

When using LPM:

1. **Always use lockfiles**: Commit `package.lock` to ensure reproducible builds
2. **Run security audits**: Use `lpm audit` to check for known vulnerabilities
3. **Keep dependencies updated**: Regularly update packages with `lpm update`
4. **Verify checksums**: LPM automatically verifies package checksums
5. **Review dependencies**: Be cautious when adding new dependencies

## Security Features

LPM includes several security features:

- **Checksum verification**: All packages are verified against checksums in the lockfile
- **Sandboxed builds**: Rust extensions are built in isolated environments
- **No postinstall scripts**: LPM does not execute arbitrary code during installation
- **OSV integration**: Automatic vulnerability scanning via OSV database
- **Secure credential storage**: Uses OS keychains for credential management

## Known Security Considerations

- LPM does not execute postinstall scripts for security reasons
- Build processes are sandboxed to limit filesystem and network access
- Package checksums are verified before installation
- Dynamic requires in Lua code are tracked and warned about

## Security Updates

Security updates will be released as patch versions following [Semantic Versioning](https://semver.org/). Critical security fixes may be backported to previous versions on a case-by-case basis.

For more information about LPM's security features, see [docs/user/Security.md](docs/user/Security.md).

