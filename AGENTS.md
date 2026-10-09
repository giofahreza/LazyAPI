# Working on LazyAPI

- After any repository changes, run the relevant checks, then rebuild and reinstall the app on this machine with `python3 scripts/install-local.py` before finishing the task.
- Verify that `command -v lazyapi` resolves to the installed build and that `lazyapi --version` succeeds. Record any installation failure explicitly.
- Use `lazyapi` as the executable and package name, `LazyAPI` in the UI, and `LAZYAPI_` for environment variables.
- Preserve the user's existing collections and environments, including saved request headers. Use temporary directories for tests and `--dry-run` when inspecting live data.
- Keep the copyright and permission notices in `LICENSE` intact.

## Checks

```sh
cargo build --release --locked --bin lazyapi
LAZYAPI_BIN=target/release/lazyapi python3 -m unittest discover -s tests -v
python3 scripts/install-local.py
```

The installer builds from this checkout, replaces the executable atomically, installs completions and the manual, and keeps the previous executable under the user's state directory. It does not modify collections or stop running app instances. Restart an open app to use a newly installed build.
