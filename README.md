# chkgithooks
A Git wrapper that verifies no local hooks are in use before invoking Git

## Usage
Compile the binary first, rename it to git, and prepend its directory to your PATH so it is prioritized during command lookup.
```
cargo build --release
mkdir -p ~/.local/bin
cp -p target/release/chkgithooks ~/.local/bin/git
export PATH="$HOME/.local/bin:$PATH"
```
