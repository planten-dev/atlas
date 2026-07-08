# 数据库设计

本文档描述当前阶段的数据库表设计。其中 `users` 表用于保存系统用户的登录身份映射，为钉钉登录提供用户映射能力，并为后续系统鉴权提供统一的用户标识。

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
| `dingtalk_user_id` | `VARCHAR(128)` | 是 | 钉钉企业通讯录真实 `userid`，用于将钉钉登录用户映射到系统内部用户。不得保存 `unionId`、`openId` 或 `uuid`。 |
| `status` | `VARCHAR(32)` | 是 | 用户状态。当前阶段用于区分用户记录是否可正常使用。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 用户记录创建时间。用户首次通过钉钉登录并创建系统用户记录时写入。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 用户记录最后更新时间。用户记录或状态发生变化时更新。 |
| `last_login_at` | `TIMESTAMPTZ` | 否 | 用户最近一次登录时间。首次创建记录时可以为空，登录成功后更新。 |

## 设计原则

- `id` 是系统内部唯一用户标识，也是 `users` 表的主键。
- `dingtalk_user_id` 用于关联钉钉企业通讯录用户，并作为钉钉登录时查找系统用户的依据。该字段必须是真实 `userid`，不是 `unionId`、`openId` 或其他网页登录身份字段。
- 系统内部逻辑统一使用 `id` 表示用户，不直接依赖钉钉用户标识。
- 用户首次通过钉钉登录时，如果不存在对应的 `dingtalk_user_id`，系统自动创建一条 `users` 记录。
- 用户后续通过钉钉登录时，系统通过 `dingtalk_user_id` 查找已有用户记录，并更新 `last_login_at`。

## 主键设计

`id` 使用 `UUID` 作为主键，用于生成稳定的系统内部用户标识。该字段不依赖外部平台标识，即使未来外部登录来源或用户映射方式发生变化，系统内部仍可以继续使用同一个 `id` 表示用户。

系统内部逻辑、日志记录和后续需要引用用户身份的地方，应统一使用 `id`。`dingtalk_user_id` 只作为外部身份映射字段使用。

## 唯一约束设计

`dingtalk_user_id` 需要设置唯一约束，保证同一个钉钉用户只能映射到一个系统用户。

该约束用于支持钉钉登录流程：

1. 登录成功后获取或换取钉钉企业通讯录真实 `userid`。
2. 使用 `dingtalk_user_id` 查询 `users` 表。
3. 如果记录存在，使用已有用户的 `id`。
4. 如果记录不存在，创建新的 `users` 记录，并生成新的 `id`。

`dingtalk_user_id` 不应作为主键使用。它来自外部平台，只适合作为登录映射字段；系统内部用户身份应由 `id` 承担。当前内部开发阶段不兼容曾错误保存为 `unionId`、`openId` 或 `uuid` 的旧数据，修复后应删除旧数据或重建开发数据库再重新登录。

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

## user_profiles 表

`user_profiles` 表是 `users` 表的一对一扩展表，用于保存用户通过钉钉登录后，从钉钉网页登录个人信息接口和钉钉“查询用户详情”接口获取并清洗后的基础资料。

`users` 表负责保存登录身份映射，例如 `dingtalk_user_id`；`user_profiles` 只保存用户资料，不重复保存钉钉用户标识，也不保存登录凭证、会话信息或未经清洗的原始资料。

### 表结构示例

```sql
CREATE TABLE user_profiles (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    name VARCHAR(128) NULL,
    avatar_url TEXT NULL,
    mobile VARCHAR(32) NULL,
    hide_mobile BOOLEAN NULL,
    telephone VARCHAR(32) NULL,
    job_number VARCHAR(64) NULL,
    title VARCHAR(128) NULL,
    email VARCHAR(255) NULL,
    org_email VARCHAR(255) NULL,
    work_place VARCHAR(255) NULL,
    remark TEXT NULL,
    department_external_ids TEXT NULL,
    is_admin BOOLEAN NULL,
    is_boss BOOLEAN NULL,
    is_active BOOLEAN NULL,
    is_senior BOOLEAN NULL,
    hired_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `user_id` | `UUID` | 是 | 用户资料所属系统用户 `id`。该字段同时作为 `user_profiles` 表主键和外键，关联 `users(id)`，与 `users.id` 一一对应。 |
| `name` | `VARCHAR(128)` | 否 | 用户姓名。钉钉接口未返回时允许为空。 |
| `avatar_url` | `TEXT` | 否 | 用户头像地址。钉钉接口未返回时允许为空。 |
| `mobile` | `VARCHAR(32)` | 否 | 用户手机号，属于个人信息。钉钉接口未返回或不可见时允许为空。 |
| `hide_mobile` | `BOOLEAN` | 否 | 钉钉侧是否隐藏手机号。接口未返回时允许为空。 |
| `telephone` | `VARCHAR(32)` | 否 | 分机号或办公电话。接口未返回时允许为空。 |
| `job_number` | `VARCHAR(64)` | 否 | 员工工号。接口未返回时允许为空。 |
| `title` | `VARCHAR(128)` | 否 | 职位或岗位名称。接口未返回时允许为空。 |
| `email` | `VARCHAR(255)` | 否 | 用户个人邮箱，属于个人信息。接口未返回时允许为空。 |
| `org_email` | `VARCHAR(255)` | 否 | 用户企业邮箱，属于个人信息。接口未返回时允许为空。 |
| `work_place` | `VARCHAR(255)` | 否 | 办公地点。接口未返回时允许为空。 |
| `remark` | `TEXT` | 否 | 钉钉用户备注信息。写入前应只保留业务需要的文本内容。 |
| `department_external_ids` | `TEXT` | 否 | 钉钉返回的 `dept_id_list`。建议保存为 JSON 字符串，避免为了该字段新增依赖；接口未返回时允许为空。 |
| `is_admin` | `BOOLEAN` | 否 | 是否为钉钉管理员。接口未返回时允许为空。 |
| `is_boss` | `BOOLEAN` | 否 | 是否为企业负责人。接口未返回时允许为空。 |
| `is_active` | `BOOLEAN` | 否 | 钉钉侧用户是否处于激活状态。接口未返回时允许为空。 |
| `is_senior` | `BOOLEAN` | 否 | 是否为高管模式用户。接口未返回时允许为空。 |
| `hired_at` | `TIMESTAMPTZ` | 否 | 入职时间。如果钉钉返回 `hired_date` 时间戳，由应用层转换为带时区时间后保存。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 用户资料记录创建时间。用户首次钉钉登录并创建资料记录时写入。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 用户资料记录最后更新时间。每次同步钉钉资料后更新。 |

### 设计原则

- `user_profiles.user_id` 与 `users.id` 一一对应，不再单独生成资料表主键。
- `user_id` 同时作为主键和外键关联 `users(id)`，建议使用 `ON DELETE CASCADE`，保证用户删除时其资料记录同步删除。
- `users` 表负责保存登录身份映射，`user_profiles` 表只保存用户基础资料。
- `dingtalk_user_id` 不应重复保存到 `user_profiles`。钉钉真实 `userid` 只保存在 `users.dingtalk_user_id`，`user_profiles` 不保存任何外部身份标识。
- `user_profiles` 不保存 `access_token`、`refresh_token`、`session id`、`client_secret` 等敏感信息。
- `user_profiles` 不保存未经清洗的 `raw_profile`，避免把 `userid`、敏感字段或无关字段再次写入资料表。
- 钉钉接口可能缺少 `mobile`、`email`、`avatar` 等字段，资料字段默认允许为空，避免因资料不完整影响用户登录。

### 钉钉登录资料同步行为

- 用户首次通过钉钉登录时，系统先从网页登录个人信息中读取真实 `userid`；如果缺少 `userid`，使用 `unionId` 换取真实 `userid`。系统随后通过 `users.dingtalk_user_id` 查找或创建 `users` 记录，再使用同一个 `users.id` 创建 `user_profiles` 记录。
- 用户再次通过钉钉登录时，系统通过 `users.dingtalk_user_id` 找到已有用户，并更新该用户对应的 `user_profiles` 记录。
- 登录时先使用网页登录个人信息接口同步头像、手机号、邮箱等可见基础资料，再使用钉钉“查询用户详情”接口补充部门、工号、职位、入职时间等组织资料。
- 同步资料时只处理明确允许保存的字段，写入前应完成字段清洗、类型转换和长度控制。
- 钉钉返回的 `dept_id_list` 保存到 `department_external_ids`，建议由应用层序列化为 JSON 字符串。
- 钉钉返回的 `hired_date` 如为时间戳，应由应用层转换后保存到 `hired_at`。
- 如果钉钉接口缺少手机号、邮箱、头像等字段，对应字段允许为空，同步流程不应因此失败。
- 每次成功写入或更新 `user_profiles` 后，应向 `events` 审计表写入 `resource_type = 'user_profiles'` 的已完成更新事件。审计事件只保存脱敏摘要，例如触发来源、同步来源、是否已有资料和更新字段名，不保存手机号、邮箱、头像地址、备注或钉钉原始响应。

### 隐私与安全约束

- 手机号、邮箱、办公电话、头像地址、备注等属于用户个人信息，日志中不应输出这些字段的明文内容。
- 登录凭证、刷新凭证、会话标识、应用密钥等敏感信息不得写入 `user_profiles`。
- 不保存未经清洗的钉钉原始响应，避免引入额外身份标识、敏感字段或当前业务不需要的字段。
- 日志只应记录必要的同步结果、用户内部 `id` 和错误类别，不记录完整用户资料。
- 写入资料表前应按字段白名单提取数据，忽略白名单之外的钉钉返回字段。

## departments 表

`departments` 表用于存储组织部门信息。部门数据可以来自钉钉同步，也可以由系统内手动创建（手动创建当前阶段尚未实现）。

### 表结构示例

```sql
CREATE TABLE departments (
    id UUID PRIMARY KEY,
    source VARCHAR(32) NOT NULL,
    external_department_id VARCHAR(128) NOT NULL,
    parent_id UUID NULL REFERENCES departments (id),
    name VARCHAR(255) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uidx_departments_source_external_department_id UNIQUE (source, external_department_id)
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 系统内部唯一部门标识，作为 `departments` 表主键。系统内部逻辑统一使用该字段识别部门。 |
| `source` | `VARCHAR(32)` | 是 | 部门来源，取值为 `dingtalk`（钉钉同步）或 `manual`（手动创建）。 |
| `external_department_id` | `VARCHAR(128)` | 是 | 外部部门标识。来源为钉钉时保存钉钉部门 `dept_id`（转为字符串）。 |
| `parent_id` | `UUID` | 否 | 上级部门 `id`，自引用外键。顶层部门为空。 |
| `name` | `VARCHAR(255)` | 是 | 部门名称。 |
| `status` | `VARCHAR(32)` | 是 | 部门状态，`active` 或 `disabled`，默认 `active`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 部门记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 部门记录最后更新时间。 |

### 设计原则

- `id` 是系统内部唯一部门标识；`external_department_id` 只作为外部映射字段使用，不作为主键。
- `(source, external_department_id)` 组合唯一，保证同一来源下的外部部门只映射到一条系统部门记录；不同来源之间外部标识互不冲突。
- `parent_id` 自引用 `departments.id` 表示部门树结构，并建立普通索引以支持按父部门查询。

### 钉钉同步行为

- 同步从钉钉根部门（`dept_id = 1`）开始逐层拉取子部门。根部门本身不入库，钉钉侧一级部门在系统中 `parent_id` 为空。
- 同步只处理新增和修改（名称、上级部门变化），**不同步删除**：钉钉侧已删除的部门在系统中保留不动。
- 新同步的部门 `status` 为 `active`；后续同步只更新名称与上级部门，不修改 `status`，手动停用的部门保持停用状态。
- 同步只影响 `source = 'dingtalk'` 的记录，手动创建的部门不受同步影响。

## systems 表

`systems` 表用于存储门店体系基础信息。

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 体系唯一标识，作为 `systems` 表主键。 |
| `name` | `VARCHAR(128)` | 是 | 体系名称。 |
| `department_id` | `UUID` | 是 | 所属本地部门 id，关联 `departments.id`。 |
| `status` | `VARCHAR(32)` | 是 | 体系状态，用于标识体系是否启用。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 体系记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 体系记录最后更新时间。 |

## systems 表设计原则

- `id` 是系统内部唯一体系标识，也是 `systems` 表的主键。
- `name` 是体系名称，不允许为空。
- `department_id` 用于存储本地部门 id，关联 `departments.id`，不允许为空。
- `status` 用于标识体系状态，例如启用、禁用。
- `created_at` 表示体系记录创建时间，创建后不应被应用逻辑主动修改。
- `updated_at` 表示体系记录最后更新时间，每次更新体系记录或状态时同步更新。
- 必须保留 `created_at` 和 `updated_at` 时间审计字段。
- 体系相关 API 使用 `systems:read` 和 `systems:write` 权限控制访问；权限策略仍由权限管理接口维护，不在 `systems` 表中存储权限关系。

## stores 表

`stores` 表用于存储门店基础信息。一个体系可以包含多个门店，一个门店只能属于一个体系。

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 门店唯一标识，作为 `stores` 表主键。 |
| `name` | `VARCHAR(128)` | 是 | 门店名称。 |
| `system_id` | `UUID` | 是 | 所属体系 ID，关联 `systems.id`。 |
| `status` | `VARCHAR(32)` | 是 | 门店状态，用于标识门店是否启用。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 门店记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 门店记录最后更新时间。 |

## stores 表设计原则

- `id` 是系统内部唯一门店标识，也是 `stores` 表的主键。
- `name` 是门店名称，不允许为空。
- `system_id` 表示门店所属体系，关联 `systems.id`，不允许为空。
- 一个体系可以包含多个门店，一个门店只能属于一个体系。
- `status` 用于表示门店启用、禁用状态，不允许为空。
- `created_at` 表示门店记录创建时间，创建后不应被应用逻辑主动修改。
- `updated_at` 表示门店记录最后更新时间，每次更新门店记录或状态时同步更新。
- `created_at` 和 `updated_at` 用于审计时间记录。
- `name` 不要求全局唯一。
- 门店相关 API 使用 `stores:read` 和 `stores:write` 权限控制访问；权限策略仍由权限管理接口维护，不在 `stores` 表中存储权限关系。

## customers 表

`customers` 表用于存储客户基础信息。当前阶段客户资料以门店、体系和部门为归属范围维护，并记录创建人、备注、附件和状态等基础字段。

客户附件字段只保存附件元数据或访问标识，不直接保存文件二进制内容。实际文件存储位置和访问权限由后续文件服务或对象存储设计承载。

### 表结构示例

```sql
CREATE TABLE customers (
    id UUID PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    creator_user_id UUID NOT NULL REFERENCES users (id),
    department_id UUID NOT NULL REFERENCES departments (id),
    system_id UUID NOT NULL REFERENCES systems (id),
    store_id UUID NOT NULL REFERENCES stores (id),
    remark TEXT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    attachments TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT customers_status_check CHECK (status IN ('active', 'disabled'))
);

CREATE INDEX idx_customers_status ON customers (status);
CREATE INDEX idx_customers_creator_user_id ON customers (creator_user_id);
CREATE INDEX idx_customers_department_id ON customers (department_id);
CREATE INDEX idx_customers_system_id ON customers (system_id);
CREATE INDEX idx_customers_store_id ON customers (store_id);
CREATE INDEX idx_customers_created_at ON customers (created_at);
CREATE INDEX idx_customers_name ON customers (name);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 客户唯一标识，作为 `customers` 表主键。 |
| `name` | `VARCHAR(128)` | 是 | 客户姓名。写入前应去除首尾空格，不能为空字符串。 |
| `creator_user_id` | `UUID` | 是 | 创建人，关联 `users.id`。展示创建人姓名、头像等信息时通过 `user_profiles` 查询。 |
| `department_id` | `UUID` | 是 | 客户所属部门，关联 `departments.id`。 |
| `system_id` | `UUID` | 是 | 客户所属体系，关联 `systems.id`。 |
| `store_id` | `UUID` | 是 | 客户所属门店，关联 `stores.id`。 |
| `remark` | `TEXT` | 否 | 客户备注，允许为空。 |
| `status` | `VARCHAR(32)` | 是 | 客户状态，允许 `active`、`disabled`，默认 `active`。 |
| `attachments` | `TEXT` | 否 | 客户附件元数据，由应用层保存为 JSON 字符串。当前 API 接收图片附件元数据数组，例如外部文件 ID、名称、MIME 类型、大小和访问标识；不直接保存图片二进制内容。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 客户记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 客户记录最后更新时间。 |

### 设计原则

- `id` 是系统内部唯一客户标识，也是 `customers` 表的主键。
- `name` 是客户姓名，不允许为空；当前阶段不要求全局唯一，也不要求在同一门店内唯一。
- 创建人字段统一关联 `users.id`；展示姓名、头像等信息时通过 `user_profiles` 查询。
- `department_id`、`system_id` 和 `store_id` 用于记录客户归属范围，均不允许为空。
- `store_id` 必须属于 `system_id`，`system_id` 应与 `department_id` 的业务归属保持一致，该类跨表业务一致性建议由服务层校验。
- `attachments` 仅保存外部图片存储返回的附件元数据或访问标识，不保存文件二进制内容、访问密钥或临时签名 URL；如后续需要复杂附件权限、版本或审计能力，再拆分独立附件表。
- 客户 API 中附件使用结构化数组表达，每个附件至少包含 `file_id`；`file_name`、`mime_type` 和 `size_bytes` 可选，提供 `mime_type` 时必须为 `image/*`。
- `remark` 和 `attachments` 可能包含客户相关敏感信息，日志中不应输出明文内容。
- 客户记录业务删除以软删除为主，需要停用时通过 `status = 'disabled'` 表示；硬删除接口仅作为管理清理能力保留。
- `created_at` 创建后不应被应用逻辑主动修改；`updated_at` 在客户记录或状态变更时同步更新。
- 客户相关 API 使用 `customers:read` 和 `customers:write` 权限控制访问；权限策略仍由权限管理接口维护，不在 `customers` 表中存储权限关系。

## products 表

`products` 表用于存储产品基础信息。产品类别通过 `category_id` 关联 `product_category.id`，系列、品牌、规格和单位当前阶段不单独建表，直接在产品记录中保存文本值。

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 产品唯一标识，作为 `products` 表主键。 |
| `name` | `VARCHAR(128)` | 是 | 产品名称。 |
| `category_id` | `UUID` | 是 | 产品类别 ID，关联 `product_category.id`。 |
| `series` | `VARCHAR(128)` | 否 | 产品系列，当前阶段使用字符串存储。 |
| `brand_name` | `VARCHAR(128)` | 否 | 品牌名称。 |
| `specification` | `VARCHAR(255)` | 否 | 产品规格。 |
| `unit` | `VARCHAR(32)` | 否 | 计量单位，用于报价、订单和业务计算。 |
| `unit_price` | `DECIMAL(12,2)` | 是 | 产品单价，使用 decimal 类型保存金额。 |
| `status` | `VARCHAR(32)` | 是 | 产品状态，允许 `active`、`disabled`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 产品记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 产品记录最后更新时间。 |

## products 表设计原则

- `id` 是系统内部唯一产品标识，也是 `products` 表的主键。
- `name` 是产品名称，不允许为空，不要求全局唯一。
- `category_id` 表示产品所属类别，不允许为空，只能关联已存在的产品类别。
- 新建或修改产品时，`category_id` 必须引用启用中的产品类别；已存在产品可以继续显示已停用类别。
- `series`、`brand_name`、`specification` 和 `unit` 允许为空，当前阶段不单独建字典表。
- `unit_price` 不允许为空，使用 decimal 类型，不使用 float 或 double，避免金额精度问题。
- `status` 用于表示产品启用、禁用状态，不允许为空。
- `created_at` 创建后不应被应用逻辑主动修改；`updated_at` 在产品记录或状态变更时同步更新。

## products 输入建议设计

- 产品类别通过 `product_category` 表维护。
- 系列、品牌和单位当前阶段不单独建表。
- 新建或编辑产品时，可以通过 `products` 表已有数据去重后提供输入建议。
- 输入建议仅作为前端辅助，不限制用户填写新值。
- 后续接口可以从 `products` 表查询 `series`、`brand_name` 和 `unit` 的 distinct 值，用于提供输入建议。

## product_category 表

`product_category` 表用于存储产品类别基础配置，维护类别名称以及该类别是否需要记录操作次数。

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 产品类别唯一标识，作为 `product_category` 表主键。 |
| `category_name` | `VARCHAR(128)` | 是 | 产品类别名称。 |
| `requires_operation_count` | `BOOLEAN` | 是 | 是否需要记录操作次数。 |
| `status` | `VARCHAR(32)` | 是 | 产品类别状态，允许 `active`、`disabled`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 产品类别记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 产品类别记录最后更新时间。 |

## product_category 表设计原则

- `id` 是系统内部唯一产品类别标识，也是 `product_category` 表的主键。
- `category_name` 不允许为空，去除首尾空格后全局唯一。
- `requires_operation_count` 用于标识该类别售出后是否需要记录可操作次数；具体次数由现场人员在销售业务中动态决定。
- `status` 用于表示产品类别启用、禁用状态，不允许为空。
- 默认初始化四个类别：`产品` 不需要操作次数，`医疗`、`仪器`、`卡项` 需要操作次数。
- 产品类别被产品引用时允许停用，但不允许物理删除。
- `created_at` 创建后不应被应用逻辑主动修改；`updated_at` 在产品类别记录或状态变更时同步更新。

## sales_records 表

`sales_records` 表用于存储具体销售成交事实。一条销售记录只对应一种销售内容类型；由前端拆分为多条销售记录，并可通过同一个 `record_group_id` 表示它们来自同一次录入。

销售内容类型复用 `product_category.id`，不在销售记录中重复维护“产品、医疗、仪器、卡项”等枚举。是否需要可操作次数由 `product_category.requires_operation_count` 控制，具体次数不直接保存在 `sales_records` 表中。

### 表结构示例

```sql
CREATE TABLE sales_records (
    id UUID PRIMARY KEY,
    record_group_id UUID NULL,
    customer_id UUID NOT NULL REFERENCES customers (id),
    department_id UUID NOT NULL REFERENCES departments (id),
    sale_date DATE NOT NULL,
    deal_status VARCHAR(32) NOT NULL,
    customer_type VARCHAR(32) NOT NULL,
    deal_type VARCHAR(32) NOT NULL,
    content_category_id UUID NOT NULL REFERENCES product_category (id),
    handler_user_id UUID NOT NULL REFERENCES users (id),
    paid_amount DECIMAL(12,2) NOT NULL DEFAULT 0.00,
    unpaid_amount DECIMAL(12,2) NOT NULL DEFAULT 0.00,
    system_id UUID NOT NULL REFERENCES systems (id),
    store_id UUID NOT NULL REFERENCES stores (id),
    collaboration_type VARCHAR(32) NOT NULL,
    expert_user_id UUID NULL REFERENCES users (id),
    expert_department_id UUID NULL REFERENCES departments (id),
    consultant_user_id UUID NULL REFERENCES users (id),
    consultant_department_id UUID NULL REFERENCES departments (id),
    doctor_user_id UUID NULL REFERENCES users (id),
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sales_records_deal_status_check CHECK (deal_status IN ('closed', 'not_closed')),
    CONSTRAINT sales_records_customer_type_check CHECK (customer_type IN ('new', 'returning')),
    CONSTRAINT sales_records_deal_type_check CHECK (deal_type IN ('non_salon', 'salon')),
    CONSTRAINT sales_records_collaboration_type_check CHECK (collaboration_type IN ('expert_consultation', 'self_sale')),
    CONSTRAINT sales_records_status_check CHECK (status IN ('active', 'voided')),
    CONSTRAINT sales_records_paid_amount_check CHECK (paid_amount >= 0),
    CONSTRAINT sales_records_unpaid_amount_check CHECK (unpaid_amount >= 0),
    CONSTRAINT sales_records_expert_consultation_check
        CHECK (
            (
                collaboration_type = 'expert_consultation'
                AND expert_user_id IS NOT NULL
                AND expert_department_id IS NOT NULL
            )
            OR
            (
                collaboration_type = 'self_sale'
                AND expert_user_id IS NULL
                AND expert_department_id IS NULL
            )
        )
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 销售记录唯一标识，作为 `sales_records` 表主键。 |
| `record_group_id` | `UUID` | 否 | 同一次录入拆分出的多条销售记录可共用该 ID，便于后续按录入批次查询或追踪。 |
| `customer_id` | `UUID` | 是 | 客户 ID，关联 `customers.id`。客户姓名、备注和附件等基础资料通过 `customers` 表查询。 |
| `department_id` | `UUID` | 是 | 成交归属部门，关联 `departments.id`。 |
| `sale_date` | `DATE` | 是 | 销售成交日期。 |
| `deal_status` | `VARCHAR(32)` | 是 | 成交状态，建议值为 `closed`、`not_closed`。 |
| `customer_type` | `VARCHAR(32)` | 是 | 客户类型，建议值为 `new`、`returning`。 |
| `deal_type` | `VARCHAR(32)` | 是 | 成交类型，建议值为 `non_salon`、`salon`。 |
| `content_category_id` | `UUID` | 是 | 销售内容类型，关联 `product_category.id`，对应产品、医疗、仪器、卡项等类别。 |
| `handler_user_id` | `UUID` | 是 | 本条销售记录处理人，关联 `users.id`。展示姓名、头像等信息时通过 `user_profiles` 查询。 |
| `paid_amount` | `DECIMAL(12,2)` | 是 | 已支付金额，默认 `0.00`，不允许为负数。 |
| `unpaid_amount` | `DECIMAL(12,2)` | 是 | 未支付金额，默认 `0.00`，不允许为负数。 |
| `system_id` | `UUID` | 是 | 成交所属体系，关联 `systems.id`。 |
| `store_id` | `UUID` | 是 | 成交所属门店，关联 `stores.id`。 |
| `collaboration_type` | `VARCHAR(32)` | 是 | 协作类型，建议值为 `expert_consultation`、`self_sale`，分别表示专家诊和自销。 |
| `expert_user_id` | `UUID` | 否 | 专家用户，关联 `users.id`。仅专家诊场景必填，自销场景应为空。 |
| `expert_department_id` | `UUID` | 否 | 专家所属部门，关联 `departments.id`。仅专家诊场景必填，自销场景应为空。 |
| `consultant_user_id` | `UUID` | 否 | 咨询师用户，关联 `users.id`，专家诊和自销场景均允许为空。 |
| `consultant_department_id` | `UUID` | 否 | 咨询师部门，关联 `departments.id`，允许为空。 |
| `doctor_user_id` | `UUID` | 否 | 医生用户，关联 `users.id`，允许为空。 |
| `status` | `VARCHAR(32)` | 是 | 销售记录状态，建议值为 `active`、`voided`，默认 `active`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 销售记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 销售记录最后更新时间。 |

### 设计原则

- `sales_records` 只记录销售成交事实，不保存操作消耗明细。
- 客户字段统一关联 `customers.id`，不在销售记录中重复保存客户姓名。
- 一条销售记录只允许一个 `content_category_id`；一次录入多个销售内容类型时，应拆分为多条销售记录。
- 销售内容类型复用 `product_category`，不重新维护“产品、医疗、仪器、卡项”枚举。
- 人员字段统一关联 `users.id`；展示姓名、头像等信息时通过 `user_profiles` 查询。
- `collaboration_type` 用于区分专家诊和自销。
- 专家和专家部门只在专家诊场景下存在；当 `collaboration_type = 'expert_consultation'` 时，`expert_user_id` 和 `expert_department_id` 必填。
- 当 `collaboration_type = 'self_sale'` 时，`expert_user_id` 和 `expert_department_id` 应为空。
- 专家与专家部门的归属一致性建议由服务层校验。
- 咨询师不是必填项，`consultant_user_id` 和 `consultant_department_id` 均允许为空。
- 咨询师字段不受 `collaboration_type` 约束；专家诊和自销都可以没有咨询师。
- 医疗、仪器、卡项等附带可操作次数的内容，不直接在 `sales_records` 中保存次数。
- 是否需要可操作次数由 `product_category.requires_operation_count` 控制。
- `paid_amount` 和 `unpaid_amount` 不允许为负数。
- `store_id` 必须属于 `system_id`，该业务一致性建议由服务层校验。
- 销售记录默认不物理删除，通过 `status = 'voided'` 表示作废。

## sales_record_operation_counts 表

`sales_record_operation_counts` 表用于保存某条销售记录生成的可操作次数账户。只有 `product_category.requires_operation_count = true` 的销售记录才需要创建该表记录，例如医疗、仪器、卡项等需要后续操作消耗的销售内容。

### 表结构示例

```sql
CREATE TABLE sales_record_operation_counts (
    sales_record_id UUID PRIMARY KEY REFERENCES sales_records (id) ON DELETE CASCADE,
    total_count INTEGER NOT NULL,
    used_count INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sales_record_operation_counts_total_count_check CHECK (total_count > 0),
    CONSTRAINT sales_record_operation_counts_used_count_check CHECK (used_count >= 0 AND used_count <= total_count),
    CONSTRAINT sales_record_operation_counts_status_check CHECK (status IN ('active', 'voided'))
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `sales_record_id` | `UUID` | 是 | 对应销售记录 ID，作为本表主键并关联 `sales_records.id`。 |
| `total_count` | `INTEGER` | 是 | 销售记录产生的总可操作次数，必须大于 `0`。 |
| `used_count` | `INTEGER` | 是 | 已使用次数，默认 `0`，不得小于 `0`，也不得超过 `total_count`。 |
| `status` | `VARCHAR(32)` | 是 | 次数账户状态，建议值为 `active`、`voided`，默认 `active`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 次数账户创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 次数账户最后更新时间。 |

### 设计原则

- `sales_record_operation_counts` 与 `sales_records` 是一对零或一关系。
- 产品类销售记录通常不创建次数账户。
- 医疗、仪器、卡项等需要操作次数的销售记录，服务层应要求填写 `total_count`。
- `used_count` 是由有效操作记录汇总维护的冗余计数字段，用于快速查询剩余次数。
- 剩余次数不单独落库，由 `total_count - used_count` 计算。
- 销售记录作废时，相关次数账户也应同步作废。
- `sales_record_id` 建议使用 `ON DELETE CASCADE`，用于在销售记录被物理删除时清理对应次数账户；正常业务仍应优先通过状态作废处理。

## sales_record_operation_usages 表

`sales_record_operation_usages` 表用于记录每一次实际操作消耗。每新增一条有效操作记录，服务层应同步增加 `sales_record_operation_counts.used_count`，从而减少剩余可操作次数。

### 表结构示例

```sql
CREATE TABLE sales_record_operation_usages (
    id UUID PRIMARY KEY,
    sales_record_id UUID NOT NULL REFERENCES sales_records (id),
    operated_at TIMESTAMPTZ NOT NULL,
    operator_user_id UUID NOT NULL REFERENCES users (id),
    doctor_user_id UUID NULL REFERENCES users (id),
    operation_count INTEGER NOT NULL DEFAULT 1,
    remark TEXT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sales_record_operation_usages_operation_count_check CHECK (operation_count > 0),
    CONSTRAINT sales_record_operation_usages_status_check CHECK (status IN ('active', 'voided'))
);
```

### 字段说明

| 字段 | 类型示例 | 是否必填 | 说明 |
| --- | --- | --- | --- |
| `id` | `UUID` | 是 | 操作消耗记录唯一标识，作为 `sales_record_operation_usages` 表主键。 |
| `sales_record_id` | `UUID` | 是 | 对应销售记录 ID，关联 `sales_records.id`。 |
| `operated_at` | `TIMESTAMPTZ` | 是 | 实际操作发生时间。 |
| `operator_user_id` | `UUID` | 是 | 本次操作记录处理人，关联 `users.id`。 |
| `doctor_user_id` | `UUID` | 否 | 本次操作医生，关联 `users.id`，允许为空。 |
| `operation_count` | `INTEGER` | 是 | 本次消耗的操作次数，默认 `1`，必须大于 `0`。 |
| `remark` | `TEXT` | 否 | 操作备注，允许为空。 |
| `status` | `VARCHAR(32)` | 是 | 操作消耗记录状态，建议值为 `active`、`voided`，默认 `active`。 |
| `created_at` | `TIMESTAMPTZ` | 是 | 操作消耗记录创建时间。 |
| `updated_at` | `TIMESTAMPTZ` | 是 | 操作消耗记录最后更新时间。 |

### 设计原则

- 每次实际操作都必须写入 `sales_record_operation_usages`，不能只修改剩余次数。
- 新增操作记录前必须校验剩余次数是否足够。
- 新增操作记录成功后，同步增加 `sales_record_operation_counts.used_count`。
- `used_count + operation_count` 不得超过 `total_count`。
- 作废操作记录时，应同步回退 `sales_record_operation_counts.used_count`。
- 操作记录默认不物理删除，通过 `status = 'voided'` 表示作废。
- 创建操作记录和更新 `used_count` 必须在同一个数据库事务中完成，避免并发下次数被超用。

## 销售记录相关索引建议

销售记录后续常见查询会围绕成交日期、客户、组织归属、门店体系、处理人、销售内容类型和操作消耗明细展开。建议在实现迁移时至少考虑以下索引：

```sql
CREATE INDEX idx_sales_records_sale_date ON sales_records (sale_date);
CREATE INDEX idx_sales_records_customer_id ON sales_records (customer_id);
CREATE INDEX idx_sales_records_department_id ON sales_records (department_id);
CREATE INDEX idx_sales_records_system_id ON sales_records (system_id);
CREATE INDEX idx_sales_records_store_id ON sales_records (store_id);
CREATE INDEX idx_sales_records_handler_user_id ON sales_records (handler_user_id);
CREATE INDEX idx_sales_records_content_category_id ON sales_records (content_category_id);
CREATE INDEX idx_sales_records_record_group_id ON sales_records (record_group_id);
CREATE INDEX idx_sales_record_operation_usages_sales_record_id ON sales_record_operation_usages (sales_record_id);
CREATE INDEX idx_sales_record_operation_usages_operated_at ON sales_record_operation_usages (operated_at);
CREATE INDEX idx_sales_record_operation_usages_operator_user_id ON sales_record_operation_usages (operator_user_id);
```
