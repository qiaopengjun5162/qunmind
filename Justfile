# QunMind - 微信群 AI 群智中枢

# 默认：运行快速检查
default: check

# ============================================================
# 代码质量
# ============================================================

# 格式化
fmt:
    cargo fmt
    taplo fmt --option reorder_keys=true Cargo.toml config.example.toml config.docker.example.toml

# 格式检查
fmt-check:
    cargo fmt --all -- --check
    taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml config.docker.example.toml

# 静态分析
clippy:
    cargo clippy --all-targets --all-features --tests --benches -- -D warnings

# 拼写检查
typos:
    typos

# 依赖审计
deny:
    cargo deny check

# ============================================================
# 测试
# ============================================================

# 全部测试
test:
    cargo nextest run --all-features

# 覆盖率摘要（快速）
coverage:
    cargo llvm-cov nextest --all-features --summary-only

# 覆盖率报告（HTML）
coverage-report:
    cargo llvm-cov nextest --all-features --html

# PostgreSQL 集成测试（需 QUNMIND_TEST_DATABASE_URL）
pg-test:
    QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" cargo nextest run --all-features --run-ignored only postgres_store

# ============================================================
# 检查组合
# ============================================================

# 快速检查（提交前）
check: fmt-check clippy test

# 完整检查（发版前）
check-all: fmt-check clippy typos deny test

# ============================================================
# 构建与运行
# ============================================================

# Debug 构建
build:
    cargo build --all-features

# Release 构建
release:
    cargo build --release

# 运行服务
run config='config.toml':
    cargo run -- --config {{config}}

# 启动 MCP server
mcp config='config.toml':
    cargo run -- --config {{config}} mcp

# ============================================================
# wx-cli 诊断
# ============================================================

# 配置体检
wxcli-doctor config='config.toml' input='':
    cargo run -- --config {{config}} wx-cli doctor {{if input != "" { "--input " + input } else { "" }}}

# 捕获消息
wxcli-capture config='config.toml' output='wx-output.json':
    cargo run -- --config {{config}} wx-cli capture --output {{output}}

# 轮询消息
wxcli-poll config='config.toml' input='':
    cargo run -- --config {{config}} wx-cli poll {{if input != "" { "--input " + input } else { "" }}}

# 预检回复触发
wxcli-dry-run config='config.toml' input='' message-id='' limit='10':
    cargo run -- --config {{config}} wx-cli dry-run \
        {{if input != "" { "--input " + input } else { "" }}} \
        {{if message-id != "" { "--message-id " + message-id } else { "" }}} \
        --limit {{limit}}

# no-send 重放（完整管线但不发微信）
wxcli-handle-once config='config.toml' input='' message-id='' limit='1':
    cargo run -- --config {{config}} wx-cli handle-once \
        {{if input != "" { "--input " + input } else { "" }}} \
        {{if message-id != "" { "--message-id " + message-id } else { "" }}} \
        --limit {{limit}} --no-send

# 渲染发送命令（不执行）
wxcli-send-dry config='config.toml' chat-id='' text='QunMind diagnostic message':
    cargo run -- --config {{config}} wx-cli send --chat-id {{chat-id}} --text "{{text}}" --dry-run

# 生成群测脚本
wxcli-test-plan config='config.toml' input='' chat-id='':
    cargo run -- --config {{config}} wx-cli test-plan \
        {{if input != "" { "--input " + input } else { "" }}} \
        {{if chat-id != "" { "--chat-id " + chat-id } else { "" }}} --shell

# ============================================================
# Docker
# ============================================================

# 构建镜像
docker-build:
    docker build --tag qunmind:local .

# Compose 配置预检
compose-config:
    docker compose config

# Compose 一键启动
compose-up:
    docker compose up -d --build

# Compose 停止
compose-down:
    docker compose down

# Compose 日志
compose-logs:
    docker compose logs -f qunmind

# Compose 状态
compose-ps:
    docker compose ps

# ============================================================
# 其他
# ============================================================

# 生成 CHANGELOG
changelog:
    git cliff -o CHANGELOG.md
