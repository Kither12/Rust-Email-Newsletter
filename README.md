# Rust-Email-Newsletter
This is a simple newsletter API built in Rust, integrating containerized deployment via Docker with PostgreSQL as a database. The system is well-tested and secure, using [lettre](https://github.com/lettre/lettre) to send emails via SMTP and [mailin-embedded](https://docs.rs/mailin-embedded/latest/mailin_embedded/) to host a mock SMTP sever for unit test purpose. 
## Pre-requisites
- [Rust](https://www.rust-lang.org/tools/install)
- [Docker](https://docs.docker.com/get-docker/)
### Windows
  
```bash
cargo install -f cargo-binutils
rustup component add llvm-tools-preview
```

```
cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres
```

### Linux
```bash
# Ubuntu 
sudo apt-get install lld clang libssl-dev postgresql-client
# Arch 
sudo pacman -S lld clang postgresql
```

```
cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres
```

### MacOS

```bash
brew install michaeleisel/zld/zld
```

```
cargo install --version="~0.7" sqlx-cli --no-default-features --features rustls,postgres
```
### 
## Usage

```bash
./script/init_db.sh
```
```bash
cargo test
```
```bash
cargo run
```
