# Certificate Fixture Templates

This directory holds non-production certificate fixture templates for local
integration and interop tests.

## Files

- `dev_client.cert.pem.example` — template client certificate payload.
- `dev_client.key.pem.example` — template private key payload.

These `.example` files are placeholders so repository scaffolding includes a
clear cert fixture contract without shipping real secrets.

## Local Generation

When a test requires parseable cert material, generate local files in this
directory with:

```bash
openssl req -x509 -nodes -newkey rsa:2048 \
  -keyout tests/fixtures/certs/dev_client.key.pem \
  -out tests/fixtures/certs/dev_client.cert.pem \
  -days 7 -subj '/CN=rustak-dev-fixture'
```
