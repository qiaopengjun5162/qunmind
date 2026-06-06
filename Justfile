# QunMind - 微信群 AI 群智中枢

# 默认：运行完整检查
default: check-all

# 格式检查
fmt-check:
    cargo fmt --all -- --check
    taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml

# 格式化
fmt:
    cargo fmt
    taplo fmt --option reorder_keys=true Cargo.toml config.example.toml

# 静态分析
clippy:
    cargo clippy --all-targets --all-features --tests --benches -- -D warnings

# 测试
test:
    cargo nextest run --all-features

# 依赖审计
deny:
    cargo deny check

# 拼写检查
typos:
    typos

# 完整检查
check-all: fmt-check clippy deny typos test

# 提交前检查
pre-commit: fmt-check clippy test

# 编译
build:
    cargo build --all-features

# 运行
run:
    cargo run

# Release 构建
release:
    cargo build --release

# 覆盖率
coverage:
    cargo llvm-cov nextest --html

# CHANGELOG
changelog:
    git cliff -o CHANGELOG.md
