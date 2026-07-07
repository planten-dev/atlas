# 数据库设计

本文档仅描述当前阶段的 `users` 表设计。该表用于保存系统用户的基础身份信息，为钉钉登录提供用户映射能力，并为后续系统鉴权提供统一的用户标识。

## users 表

`users` 表是系统用户的基础表。当前阶段只保存系统用户身份识别所需字段。

### 表结构示例

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    dingtalk_user_id VARCHAR(128) NOT NULL UNIQUE,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ NULL,
    CONSTRAINT users_status_check CHECK (status IN ('active', 'disabled'))
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 系统内部唯一用户标识，作为 `users` 表主键。系统内部逻辑统一使用该字段识别用户。 |
| `dingtalk_user_id` | `VARCHAR(128)` | 是 | 钉钉用户标识，用于将钉钉登录用户映射到系统内部用户。 |
| `status` | `VARCHAR(32)` | 是 | 用户状态。当前阶段用于区分用户记录是否可正常使用。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 用户记录创建时间。用户首次通过钉钉登录并创建系统用户记录时写入。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 用户记录最后更新时间。用户记录或状态发生变化时更新。 |
| `last_login_at` | `TIMESTAMPTZ` | 否 | 用户最近一次登录时间。首次创建记录时可以为空，登录成功后更新。 |

## 设计原则

- `id` 是系统内部唯一用户标识，也是 `users` 表的主键。
- `dingtalk_user_id` 用于关联钉钉用户，并作为钉钉登录时查找系统用户的依据。
- 系统内部逻辑统一使用 `id` 表示用户，不直接依赖钉钉用户标识。
- 用户首次通过钉钉登录时，如果不存在对应的 `dingtalk_user_id`，系统自动创建一条 `users` 记录。
- 用户后续通过钉钉登录时，系统通过 `dingtalk_user_id` 查找已有用户记录，并更新 `last_login_at`。

## 主键设计

`id` 使用 `UUID` 作为主键，用于生成稳定的系统内部用户标识。该字段不依赖外部平台标识，即使未来外部登录来源或用户映射方式发生变化，系统内部仍可以继续使用同一个 `id` 表示用户。

系统内部逻辑、日志记录和后续需要引用用户身份的地方，应统一使用 `id`。`dingtalk_user_id` 只作为外部身份映射字段使用。

## 唯一约束设计

`dingtalk_user_id` 需要设置唯一约束，保证同一个钉钉用户只能映射到一个系统用户。

该约束用于支持钉钉登录流程：

1. 登录成功后获取钉钉用户标识。
2. 使用 `dingtalk_user_id` 查询 `users` 表。
3. 如果记录存在，使用已有用户的 `id`。
4. 如果记录不存在，创建新的 `users` 记录，并生成新的 `id`。

`dingtalk_user_id` 不应作为主键使用。它来自外部平台，只适合作为登录映射字段；系统内部用户身份应由 `id` 承担。

## 时间字段设计

`created_at`、`updated_at` 和 `last_login_at` 都使用带时区的时间类型，例如 PostgreSQL 的 `TIMESTAMPTZ`，避免不同运行环境下出现时间解释不一致的问题。

- `created_at` 表示用户记录创建时间，创建后不应被应用逻辑主动修改。
- `updated_at` 表示用户记录最后更新时间，每次更新用户记录或状态时同步更新。
- `last_login_at` 表示用户最近一次登录时间。该字段允许为空，用于兼容用户记录已创建但尚未完成登录时间写入的场景。

## 用户状态字段设计

`status` 用于记录用户当前状态。当前阶段建议使用简单明确的字符串值，并通过约束限制可选范围。

当前建议状态值：

| 状态 | 说明 |
| --- | --- |
| `active` | 用户记录处于正常状态，可作为有效系统用户使用。 |
| `disabled` | 用户记录已被停用，不应作为正常用户继续使用。 |

`status` 默认值为 `active`，用户首次登录自动创建记录时默认进入正常状态。后续如需要停用用户，只更新该字段，不改变用户的 `id` 或 `dingtalk_user_id` 映射关系。
