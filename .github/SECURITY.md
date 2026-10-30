# Security Policy

## Reporting a vulnerability

Do not open a public GitHub issue for a security vulnerability.

Report it privately: on the repository's **Security** tab, select **Report a vulnerability**. Give the details and the steps to reproduce it.

We will acknowledge your report. We will work with you to understand and fix the problem before any public disclosure.

## Scope

Hya is a library, a WebAssembly package, and a command-line tool. It makes no network requests, and it runs no server. Only the latest release is supported. There are no older supported versions.

In scope:

- A panic, a hang, or unbounded memory use for any input that the public API accepts. The core is meant to be total: every input returns a schedule or a typed error.
- An exception other than `PrayerTimesError` escaping the JS package.
- The CLI reading or writing outside its own config file.
- A problem in the build or release pipeline, such as a dependency or a GitHub Actions workflow.

A wrong prayer time is not a security issue. Report it in public with the **Timetable mismatch** form, so others can check it.
