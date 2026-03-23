import type { Meta, StoryObj } from '@storybook/react-vite'
import { Markdown } from './Markdown'

const meta = {
  title: 'Chat/Shared/Markdown',
  component: Markdown,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Markdown>

export default meta
type Story = StoryObj<typeof meta>

export const Paragraph: Story = {
  args: { content: 'This is a simple paragraph with **bold**, *italic*, and `inline code`.' },
}

export const CodeBlock: Story = {
  args: {
    content: `Here's a Rust code example:

\`\`\`rust
pub struct TokenValidator {
    jwks: JwkSet,
    cache: LruCache<String, Claims>,
}

impl TokenValidator {
    pub fn validate(&mut self, token: &str) -> Result<Claims> {
        if let Some(claims) = self.cache.get(token) {
            return Ok(claims.clone());
        }
        let claims = self.verify_jwt(token)?;
        self.cache.put(token.to_string(), claims.clone());
        Ok(claims)
    }
}
\`\`\``,
  },
}

export const Table: Story = {
  args: {
    content: `| Module | Lines | Status |
|--------|-------|--------|
| auth/middleware.rs | 120 | Refactored |
| auth/validator.rs | 180 | New |
| auth/session.rs | 90 | New |`,
  },
}

export const Lists: Story = {
  args: {
    content: `### Ordered list
1. Read the current implementation
2. Extract token validation
3. Add caching
4. Update tests

### Unordered list
- JWT verification
- Session management
- Rate limiting
- Permission checking`,
  },
}

export const Blockquote: Story = {
  args: {
    content: `> **Note**: The cache TTL should match the JWT expiry to avoid serving stale claims.

> **Warning**: Never store raw tokens in the cache — always hash first.`,
  },
}

export const MixedContent: Story = {
  args: {
    content: `## Authentication Refactoring

The middleware currently handles **4 concerns** in a single module. Let me break them apart:

1. **Token validation** → \`auth/validator.rs\`
2. **Session management** → \`auth/session.rs\`

### Before

\`\`\`rust
// 450 lines of spaghetti
fn middleware(req: Request) -> Response {
    // everything here...
}
\`\`\`

### After

| Module | Responsibility | Lines |
|--------|---------------|-------|
| \`validator.rs\` | JWT + caching | 180 |
| \`session.rs\` | Session lifecycle | 90 |
| \`middleware.rs\` | Composition | 120 |

> This reduces the median PR review time from **45 min** to **15 min** per change.`,
  },
}
