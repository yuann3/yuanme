# setup-yuanme

Installs an exact [yuanme](https://github.com/yuann3/yuanme) release in a GitHub Actions job and puts it on `PATH`.

```yaml
- uses: yuann3/setup-yuanme@<commit SHA> # vN
  with:
    version: 0.2.0
- run: yuanme build --json
```

- **`version`** is required and exact, `X.Y.Z`: a project's builds stay on the release it names until you change it. `yuanme self update` reminds you to.
- **The archive is checked** with `gh attestation verify --repo yuann3/yuanme`, using the runner's `gh` and the job's token, before anything is unpacked. The binary must then report `yuanme X.Y.Z`.
- **The binary is cached** per version and runner with `actions/cache`, and only once it is checked. A cached run downloads nothing.
- It runs on Linux, macOS and Windows, on x86_64 and arm64; Linux gets the static musl binary.

| Input | Default | |
|---|---|---|
| `version` | | The release, such as `0.2.0` |
| `token` | `github.token` | What `gh attestation verify` reads the attestation with |
| `release-url` | `https://github.com/yuann3/yuanme/releases/download` | Where `vX.Y.Z/<archive>` is; only the action's own test changes it |

| Output | |
|---|---|
| `path` | The folder holding `yuanme`, which is on `PATH` |
| `cache-hit` | `true` when the binary came from the cache |

The action lives in `actions/setup-yuanme/` of the yuanme repository, where `.github/workflows/setup-yuanme.yml` tests it, until it has a repository of its own.
