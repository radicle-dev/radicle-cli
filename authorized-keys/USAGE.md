# `rad-auth-keys -h`

```
rad-auth-keys 0.1.0
Radicle authorized keys CLI tool for managing Radicle git repository authorized keys.

USAGE:
    rad-auth-keys [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -d, --dir <dir>              Optional, the path for parent directory of `.rad/` directory, defaults to
                                 `std::env::current_dir()?`
    -i, --id <id>                Optional, the signing key id (fingerprint) to add or remove to authorized keys list;
                                 required for `remove`
    -k, --key-type <key-type>    Optional, the signing key type used for authenticating a request, e.g. `openpgp`,
                                 `eip155:<chain_id>`, `ed25519`; defaults to `openpgp`
    -p, --path <path>            Optional, the path to the public key, otherwise will accept standard input for the
                                 public key
    -s, --source <source>        Optional, the source of the keyring to modify, e.g. `radkeys`, `radid`, `ens`; defaults
                                 to `radkeys`

SUBCOMMANDS:
    add       add a public key to the keyring source
    help      Prints this message or the help of the given subcommand(s)
    list      list the public keys in the keyring source
    remove    remove a public key from the keyring source
```

## Exporting PGP Public Key to `rad-auth-keys`

Exporting a PGP public key to `rad-auth-keys` is as simple as piping the `gpg` export to `rad-auth-keys`, e.g.:

```
gpg --armor --export <your-email> | rad-auth-keys add
```

## Using the `--key-path` Argument to Add a PGP Public Key

If the pgp public key exists on file, the `--key-path` or `-k` argument can be used to add the public key to the keyring, e.g.:

```
rad-auth-keys add -t openpgp -p path/to/public/key.pub
```

## Manually Adding a Key

It is also possible to manually add a `.rad/keys/` key without using the `rad-auth-keys` CLI helper.

`.rad/keys/openpgp/<fingerprint>`

The contents of fingerprint would be a PGP public key block, e.g.:

```
-----BEGIN PGP PUBLIC KEY BLOCK-----

mQENBF6HnfIBCADVkmIpkcVy/xcDlCh4UjjSl69a5lmqd6t/D/HK9ywnmVwtu47Q
A+Fa8g5ku8txTxGdTAgKtUdGnsvs6UjKfXSpe5nvoMlDNy5eO6qgf2ZQ0hNdQZtd
jJreVDxUftfbLZXP6jqDUNH4y2X5R2JFvBBCsd0NliGwFp5wImOZEfUJpz+u0cb6
FbNnNI1PkboHbOTo3iKYP4PaBn5ARls6HxTFQ9JoayE7Wubk/HGK8GjrTeFni/Ku
a7jCSxWCeHR3smlnwtBwFKG5BEd7cmbYbnUWu6N1qW7tGPT/Duv+++DdW8HMgjec
TWM6ED7cA/3A8jiCU7g5bLiB2pQJlyIUEGybABEBAAG0J1J5YW4gVGF0ZSA8cnlh
bi5taWNoYWVsLnRhdGVAZ21haWwuY29tPokBVAQTAQgAPhYhBKJrbDjPA+cyuV6B
pIF+7+MuHwqlBQJeh53yAhsDBQkDwmcABQsJCAcCBhUKCQgLAgQWAgMBAh4BAheA
AAoJEIF+7+MuHwqlINUIALyuI1UyAh9+GLLH6L8kdlESoDRaocECJ8sJ0m5LdfaH
NC3cQHwd3OV/pxE8xLCkvKn7sGAs0ar3xIHMb/hiEUM7cSsQV8ZhhhZDNWh3PSG9
u7i5QH0ip4p0P4A6KM250PdULYo8bQ3oxFfRCUN1pTheFMZDoCcKPBC2h/bazV0O
Qyhw7Qi2BT+Vmy3K+qiaoydGnlltz1EzKIZKx53WS+jv2qE+jMp66zr2DuN/alas
tqTgCr2IjkLAsh48Lfy9oY4sims++7TnGsjpOL3PqENyNQ8yDBB0yi+kEi93pL4J
dkyOyGMKxUAqP1bf18sN2tv64jcvlOVcPCyPRsbJiRK5AQ0EXoed8gEIANJaqA4o
eZFz2EqvaNYP1a/cKDCFAZSFIYgD4EMaFDXv72vLKfzGKcJVoX1lXp55V+3qf1Eh
TRqsKWQFLBljgcEH43A3sxdr7eGbZnVbOC8wa1YoQxgvERJwJKXIowAtoinJqZEW
3DKqb5WrMX7GLzqyymR+KETU/BhqeKpGqdzNH2dwOgCfwsHesd3vrPiAsQfFbZ1E
GOn/6TRyKwgM8QRF81eGKDkv71NLYEKFZWfmELN1C0VLB7Bs9Snpw6qd3DWqMMqF
YYwFzyR3NUfTq8Sw5BmjC0jB1ePx/U/HyiozYEp5+TwZ0+zN6sAU0/Gun8se+Fv2
S2MCq2BKKyguT5cAEQEAAYkBPAQYAQgAJhYhBKJrbDjPA+cyuV6BpIF+7+MuHwql
BQJeh53yAhsMBQkDwmcAAAoJEIF+7+MuHwql8c8H/jIzKGBU7ahVQkV76j086K3Z
/1CUA6N+6BqnwF4mvnxrnk6sw7pgy1XLLxWbh3ERPVHoCG5NHZmkFb/KeOd4bsKB
WKKhXyQosVnJA20DTq8gkleyE5qsyCLNd9YlvH2+jaNuL6xBii+ucwWZxI+6g9pY
s5W7L6Q9CXYH6HGFKc/ruIPA0+TJhpAddPtzX8JtxvY3OpV5zMhrIdXLOidCu0cb
HEEgb5jsomfiRHdFmprjQN7q5YePXQKzp8QToNi2zJBxmS4W9mllhizHRFhKATZQ
5Xlgd6JWKqzVlXAQpR/cRUJCnqMt7KTURJt1YOTFnYEb9swXEUW8AzCjw41eEV4=
=9Cll
-----END PGP PUBLIC KEY BLOCK-----
```

The `gpg --armor --export <email>` command can be used to get the public key block.
