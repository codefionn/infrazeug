# hello-multi-playbook

Two playbooks in one binary, selected with `--playbook`:

- `main` (default) — `nginx -v` on localhost
- `machines` — `uname -m` on localhost

```bash
cargo run -p hello-multi-playbook -- plan
cargo run -p hello-multi-playbook -- plan --playbook machines
cargo run -p hello-multi-playbook -- graph --playbook machines
```
