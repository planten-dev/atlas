## users

### 字段

| 字段 | 类型 | 约束 | 说明 |
| --- | --- | --- | --- |
| id | UUID | PRIMARY KEY | 系统内部唯一标识 |
| username | VARCHAR(64) | NOT NULL, UNIQUE | 用户名，可用于登录 |
| phone | VARCHAR(20) | NOT NULL, UNIQUE | 手机号，可用于登录 |
| password_hash | TEXT | NOT NULL | 密码哈希值 |
| status | ENUM('active', 'disabled') | NOT NULL, DEFAULT 'active' | 账号状态 |
| created_at | TIMESTAMP | NOT NULL | 创建时间 |
| updated_at | TIMESTAMP | NOT NULL | 更新时间 |

### 约束

- id 为主键。
- username 唯一且不能为空。
- phone 唯一且不能为空。
- password_hash 不允许为空。
- 不存储明文密码。
- password_hash 不返回给客户端。
- 所有关联关系均使用 id 作为外键。
