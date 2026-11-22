# `dev/`

`service-account-key.dev.json` is **not a real credential**. It reuses the
throwaway RSA key already committed at
`backend/tests/fixtures/test_rsa_private_key.pem` (generated for unit tests,
never used against real Google), with `token_uri` pointed at the local
`sheets-stub` container from `docker-compose.dev.yml` instead of Google's
real token endpoint. Safe to commit; there is nothing to leak.
