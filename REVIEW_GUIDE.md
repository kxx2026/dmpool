# DMPool 代码审查指南

## 项目概述
DMPool 是基于 Hydrapool 的比特币挖矿池衍生项目

## 技术栈
- Rust 1.88.0+
- Tokio, Bitcoin 0.32.5, RocksDB, Axum

## 代码审查清单
- 无 unwrap/expect panic
- 使用 Result<> 错误传播  
- 有对应测试
- AGPLv3 合规

## 测试命令
cargo test

## 分支说明
- main: 稳定版本
- dev: 开发分支（包含新功能和文档）

---
版本: v1.0 | 2025-01-31 | dev 分支专用
