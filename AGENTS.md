# Agent Guidelines for flexplus

This document contains guidelines for AI coding agents working in this repository.

## Tech Stack

- **Monorepo:** Turborepo with Bun workspaces
- **Frontend:** Next.js 16, React 19, TailwindCSS 4, shadcn/ui
- **Backend:** tRPC, Better Auth, Drizzle ORM, PostgreSQL
- **Python API:** FastAPI (energy proxy) with uv/pytest
- **Tooling:** Biome (lint/format), Bun (runtime/package manager/test runner)
- **Infrastructure:** AWS (ECS, Aurora, ALB), OpenTofu/Terraform

## Bun - Primary JavaScript Runtime

This project uses **Bun** as the JavaScript/TypeScript runtime, package manager, and test runner. Do NOT use npm, yarn, pnpm, node, jest, or vitest.

### Key Bun Commands

```bash
bun install                  # Install dependencies (NOT npm install)
bun run <script>             # Run package.json scripts
bun test                     # Run tests using Bun's native test runner
bun test path/to/file.test.ts  # Run specific test file
bun <file.ts>                # Execute TypeScript directly
```

### Bun Test Runner

Tests use Bun's native test runner, NOT Jest or Vitest. Always import from `"bun:test"`:

```typescript
// ✅ CORRECT - Bun's native test runner
import { describe, expect, it, mock, beforeAll, afterAll } from "bun:test";

// ❌ WRONG - Do not use these
import { describe, expect, it } from "jest";      // NO
import { describe, expect, it } from "vitest";    // NO
import { describe, expect, it } from "@jest/globals"; // NO
```

### Why Bun?

- Faster than Node.js for most operations
- Native TypeScript execution (no transpilation needed)
- Built-in test runner with Jest-compatible API
- Built-in package manager (faster than npm/yarn)
- Workspace support for monorepos

## Build, Lint, Test Commands

### Root-level commands (all workspaces)

```bash
bun install                  # Install all dependencies
bun run dev                  # Start all apps in dev mode
bun run build                # Build all packages and apps
bun run check-types          # TypeScript type checking
bun run check                # Biome lint + format (auto-fix)
bun test                     # Run all tests (sets up test DB first)
bun test:ci                  # Run tests in CI (no DB setup)
```

### Single test execution

For TypeScript/JavaScript tests (using Bun's test runner):

```bash
bun test path/to/file.test.ts                    # Run single test file
bun test --test-name-pattern "pattern"           # Run tests matching pattern
```

For Python tests (energy-api):

```bash
make -C apps/energy-api test-path PATH=tests/unit/test_agent.py
# Or from apps/energy-api:
uv run pytest tests/unit/test_agent.py
uv run pytest tests/unit/test_agent.py::test_function_name
```

### Package-specific commands

```bash
# Web app
turbo -F web dev             # Dev server at http://localhost:3000
turbo -F web build           # Production build

# Database
bun run db:push              # Push schema changes to DB
bun run db:studio            # Open Drizzle Studio GUI
bun run db:generate          # Generate migration files
bun run db:migrate           # Run migrations

# energy API (from root)
make energy-api-dev      # Start FastAPI dev server
make energy-api-test     # Run Python tests
make energy-api-lint     # Run Ruff linter
make energy-api-format   # Format with Ruff
```

## Code Style & Formatting

### Biome Configuration

- **Indentation:** Tabs (not spaces)
- **Quote style:** Double quotes for JS/TS
- **Line length:** Default (Biome standard)
- **Import organization:** Auto-organized with `organizeImports: "on"`
- **Self-closing elements:** Required (`useSelfClosingElements: error`)
- **Template literals:** No unnecessary template literals

### Python (Ruff)

- **Line length:** 120 characters
- **Import sorting:** isort-compatible (`ruff check --select I --fix`)
- **Format:** `ruff format`

### TypeScript

- **Strict mode:** Enabled (all strict checks)
- **No unchecked indexed access:** `noUncheckedIndexedAccess: true`
- **No unused locals/parameters:** Enforced
- **Module resolution:** `bundler` (supports workspace imports)
- **Target:** ESNext

### Import Conventions

```typescript
// 1. External packages (from node_modules)
import { describe, expect, it } from "bun:test";
import { initTRPC } from "@trpc/server";

// 2. Workspace packages (with @flexplus/ prefix)
import { createDb } from "@flexplus/db";
import type { UserRepository } from "@flexplus/api/modules/user/infrastructure/repositories/user-repository/user.repository";

// 3. Relative imports (prefer absolute workspace imports when possible)
import { TestHarness } from "../test/test-harness";
import type { Context } from "./context";
```

## Naming Conventions

### Files & Folders

- **Components:** `kebab-case.tsx` (e.g., `user-profile.tsx`)
- **Test files:** `*.test.ts` or `*.test.tsx`
- **Config files:** `kebab-case` (e.g., `next.config.ts`)
- **Folders:** `kebab-case` (e.g., `user-repository/`)
- **Schema files:** `*.schema.ts`
- **Use case files:** `action-name.use-case.ts`

### TypeScript

```typescript
// Types & Interfaces: PascalCase
type UpdateUserProfileInput = ProfileInput & { userId: string };
interface UserRepository { ... }

// Classes: PascalCase
class UpdateUserProfileUseCase { ... }

// Functions/variables: camelCase
const displayName = ...;
async function updateProfile() { ... }

// Constants: camelCase (not UPPER_CASE unless truly constant)
const maxRetries = 3;

// Enums: PascalCase for enum, UPPER_CASE for values (if applicable)
```

## Type Safety & Error Handling

### Use `type` for type imports

```typescript
import type { UserRepository } from "..."; // Preferred
```

### Strict null checks

```typescript
// Always handle nullable types explicitly
const user = await repository.findById(userId);
if (!user) throw new Error("User not found"); // Don't assume non-null
```

### Type narrowing with type guards

```typescript
const nameFromParts = [firstName, lastName]
	.filter((value): value is string => value !== null) // Type guard
	.join(" ");
```

### Error handling in procedures

```typescript
import { TRPCError } from "@trpc/server";

throw new TRPCError({
	code: "UNAUTHORIZED",
	message: "Authentication required",
	cause: "No session",
});
```

## Database & Testing

### Test database setup

Tests use a separate test database (`postgres-test` on port 5433):

```bash
bun run db:test:setup       # Start test DB + push schema
docker compose up -d postgres-test  # Just start container
```

### Test patterns

```typescript
import { afterAll, afterEach, beforeAll, describe, expect, it, mock } from "bun:test";
import { TestHarness } from "../test/test-harness";

describe("FeatureName", () => {
	// Unit test with mocks
	it("should handle specific case", async () => {
		const mockFn = mock(async () => ({ id: "1" }));
		const mockRepo = { updateProfile: mockFn } as unknown as UserRepository;
		// ... assertions
	});

	// Integration test block
	describe("integration with Repository", () => {
		beforeAll(() => {
			// Setup real dependencies
		});

		afterEach(async () => {
			await TestHarness.clearDatabase();
		});

		afterAll(async () => {
			await TestHarness.teardown();
		});

		it("updates database correctly", async () => {
			const user = await TestHarness.createTestUser({ name: "Test" });
			// ... test with real DB
		});
	});
});
```

## Common Patterns

### tRPC procedures

```typescript
export const protectedProcedure = t.procedure.use(({ ctx, next }) => {
	if (!ctx.session) {
		throw new TRPCError({ code: "UNAUTHORIZED", message: "..." });
	}
	return next({ ctx: { ...ctx, session: ctx.session } });
});
```

### Use case pattern

```typescript
export class ActionNameUseCase {
	constructor(private readonly repository: Repository) {}

	async execute(input: Input): Promise<Output> {
		// Validation, business logic, repository calls
	}
}
```

## Additional Notes

- Prefer `const` over `let`
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid parameter reassignment (enforced by Biome)
- Use `as const` assertions where appropriate
- No inferrable types in annotations (enforced)
- Always use `async/await` (no floating promises - enforced)
- Use template literals only when necessary

# general

1. When working in an existing project, first inspect and follow that project's established design patterns and architecture. If those patterns conflict with these global rules, ask which pattern to follow for that project before changing code.
2. When refactoring code make sure to tidy up after yourslef and look at old references and implementations to understand the context anbd tidy up old logic that is either no longer used and/or no longer needed/outdated and needs updating to the new pattern.
3. The local machine is MacosX with zsh as default shell.
4. The local machine cpu arch is ARM (M4 max chip).
5. for building an ddeploying docker containers for remote purposes we most likely should use x86 build by default.
6. I am on macos and use zsh so make sure cli commands work for this. also ensure that usally remote is some linux with bash as default shell so things need to also work there.
7. try use ripgrep for searching and fallback to grep. again locally i am on macos so grep is different from gnu grep.
8. Always use context7 when I need code generation, setup or configuration steps, or library/API documentation. This means you should automatically use the Context7 MCP tools to resolve library id and get library docs without me having to explicitly ask.
9. Do NOT be syccophantic. Be bold and ask questions if you are not sure about something. Show scrutiny and think critically.

# python

1. prefer functional-style and dataclasses over object oriented programming
2. use type hints
3. use uv for venv and dependency management; do NOT use poetry
4. Use uv_build as build tool instead of setup tools.
5. use black for formatting via uv ruff and pyproject.toml
6. use isort for imports via uv ruff and pyproject.toml
7. use pyrefly for type checking
8. use pytest for testing
9. use ruff for linting
10. use ruff for formatting: linelength shoudl be 120 by default.
11. use uv and conventional commits pre-commit hooks
12. for anything http call related use httpx where possible as the preferred lib to do so.
13. use list and dictionary comprehensions and itertools as much as possibel instead of for loops or while loops.
14. when writing script for CLI use use typer package and modern python
15. pytest doesnt need classes as test case definitions. they can be handled via files and functioned and fixtures should be used 

# typescript

1. prefer const style over functions for functions
2. Use turbo repo and monorepo structure based on turborepo as a preference
3. prefer interfaces and implements over classes
4. use bun for running scripts
5. for React prefer const arrow functional components as opposed to classes or functions
6. use bun for formatting via `bun run check` (runs Biome)
7. use bun for linting via `bun run check` (runs Biome)
8. use Bun's native test runner (`bun:test`) - NOT Jest. Import from `"bun:test"` not `"jest"` or `"vitest"`
9. use bun and conventional commits pre-commit hooks
10. in react prefer const arrow functional components as opposed to classes or functions
11. in react prefer small, reusable components
12. for react projects use typescript and use @ style for imports where possible. 

# general

1. When working in an existing project, first inspect and follow that project's established design patterns and architecture. If those patterns conflict with these global rules, ask which pattern to follow for that project before changing code.
2. When refactoring code make sure to tidy up after yourslef and look at old references and implementations to understand the context anbd tidy up old logic that is either no longer used and/or no longer needed/outdated and needs updating to the new pattern.

# Logic Refactoring Rules

Based on ArjanCodes' logic refactoring examples showing progression from nested conditionals to clean, testable code.

## Guard Clauses

- Use early returns for exceptional/rejection cases instead of nested if-statements
- Check for privilege/override conditions first (e.g., `if user.is_admin: return "approved"`)
- Then check for rejection conditions with early returns
- The "happy path" should be at the end with minimal indentation

```python
# Bad: nested conditionals
if user.is_premium:
    if order.amount > 1000:
        if not order.has_discount:
            return "approved"

# Good: guard clauses
if not user.is_premium:
    return "rejected"
if order.amount <= 1000:
    return "rejected"
if order.has_discount:
    return "rejected"
return "approved"
```

## Named Boolean Conditions

- Extract complex boolean expressions into well-named functions
- Function names should read like natural language questions
- Keep functions focused on a single logical concept

```python
# Bad: inline complex condition
if order.amount > 1000 or (order.type == "bulk" and not user.is_trial):
    return "approved"

# Good: named condition
def is_eligible_amount(order: Order, user: User) -> bool:
    return order.amount > 1000 or (order.type == "bulk" and not user.is_trial)

if is_eligible_amount(order, user):
    return "approved"
```

## Loop Simplification

- Replace explicit loops with `any()` or `all()` for boolean checks
- Use generator expressions inside `any()`/`all()` for readability

```python
# Bad: explicit loop
for item in order.items:
    if item.price < 0:
        return "rejected"

# Good: any() with generator
if any(item.price < 0 for item in order.items):
    return "rejected"
```

## Merging Related Logic

- Group related rules into a collection when they share structure
- Use lambda functions or callable lists for similar validation patterns

```python
# Bad: repetitive if statements
if not user.is_premium:
    return "rejected"
if order.amount is None:
    return "rejected"
if order.has_discount:
    return "rejected"

# Good: merged rules
rejection_rules = [
    lambda: not user.is_premium,
    lambda: order.amount is None,
    lambda: order.has_discount,
]

if any(rule() for rule in rejection_rules):
    return "rejected"
```

## Data-Driven Logic

- Extract hardcoded conditional logic into data structures
- Use dictionaries or constants for mappings
- Makes rules easier to modify and test

```python
# Bad: hardcoded conditionals
if user.region == "EU" and order.currency != "EUR":
    return "rejected"
if user.region == "US" and order.currency != "USD":
    return "rejected"

# Good: data-driven
VALID_REGION_CURRENCY = {
    ("EU", "EUR"): True,
    ("US", "USD"): True,
    ("UK", "GBP"): True,
}

def has_valid_currency(order: Order, user: User) -> bool:
    return VALID_REGION_CURRENCY.get((user.region, order.currency), False)
```

## Exception Handling

- Avoid blanket try-except blocks that hide real problems
- Handle specific exceptions only when necessary
- Validate inputs explicitly instead of relying on exceptions
- Check for None/invalid data with explicit conditionals

```python
# Bad: hiding all errors
try:
    if order.amount > 1000:
        return "approved"
except Exception:
    return "rejected"

# Good: explicit validation
if order.amount is None:
    return "rejected"
if order.amount > 1000:
    return "approved"
```

## Testing Strategy

- Write characterization tests BEFORE refactoring complex logic
- Create test helpers for object creation with sensible defaults
- Test both positive and negative cases
- Test edge cases (negative values, None, boundary conditions)
- Each refactoring step should keep tests passing

```python
def make_order(**kwargs):
    """Helper to create an order with sane defaults."""
    defaults = dict(
        amount=1200,
        has_discount=False,
        region="US",
        currency="USD",
        type="normal",
        items=[Item("Keyboard", 100.0)],
    )
    defaults.update(kwargs)
    return Order(**defaults)
```

## Code Structure Principles

1. **Flatten nested conditionals** - Maximum 2 levels of indentation in business logic
2. **Use descriptive function names** - `is_eligible_amount()`, `has_valid_currency()`
3. **Extract helper functions** - When a condition is complex or reused
4. **Group similar checks** - Related validations go together
5. **Privilege checks first** - Handle override/admin cases at the top
6. **Type hints everywhere** - Use proper return types and parameter types

## Refactoring Process

Follow this progression when cleaning up messy logic:

1. **Write characterization tests** - Lock in current behavior
2. **Apply guard clauses** - Convert nested ifs to early returns
3. **Remove unnecessary try-except** - Replace with explicit checks
4. **Name complex conditions** - Extract into functions
5. **Simplify loops** - Use `any()`/`all()` where appropriate
6. **Merge similar logic** - Group related rules
7. **Make logic data-driven** - Extract hardcoded rules to constants

## What to Avoid

- ❌ Deep nesting (more than 2-3 levels)
- ❌ Mixing privilege checks with business rules
- ❌ Complex boolean expressions without names
- ❌ Blanket exception catching
- ❌ Hardcoded business rules scattered in conditionals
- ❌ Explicit loops for simple boolean checks
- ❌ Implicit behavior (be explicit about what you're checking)

## What to Do

- ✅ Use early returns (guard clauses)
- ✅ Name your conditions with helper functions
- ✅ Use `any()`/`all()` for collection checks
- ✅ Group related validation rules
- ✅ Extract configuration to constants/data structures
- ✅ Check privilege/override cases first
- ✅ Make every condition easy to read and understand

## Claude Code Integration

This project uses Claude Code skills, hooks, and a Python orchestrator for autonomous migration workflows.

### Structure

```text
.claude/
├── CLAUDE.md              # Project memory/context
├── settings.json          # Hook configuration
├── scripts/
│   ├── orchestrator.py    # Task logic (single source of truth)
│   └── hook.sh            # Hook wrapper
├── skills/                # Skill definitions
│   ├── implement/
│   │   ├── SKILL.md       # Main skill
│   │   ├── examples.md    # Code examples
│   │   └── task-template.md
│   ├── status/
│   │   └── SKILL.md
│   └── validate/
│       ├── SKILL.md
│       └── checklist.md
└── state/
    └── progress.json      # Runtime state (gitignored)
```

### Skills (Slash Commands)

| Skill | Model | Purpose |
|-------|-------|---------|
| `/implement` | opus | Autonomous task implementation with examples |
| `/status` | haiku | Progress, dependency graph, checkpoints |
| `/validate` | sonnet | Standards, security, docker, tests |

### Orchestrator CLI

```bash
uv run python .claude/scripts/orchestrator.py next       # Get next task(s)
uv run python .claude/scripts/orchestrator.py status     # Show progress
uv run python .claude/scripts/orchestrator.py graph      # Dependency graph
uv run python .claude/scripts/orchestrator.py details X  # Task details
uv run python .claude/scripts/orchestrator.py complete X # Mark complete
uv run python .claude/scripts/orchestrator.py fail X msg # Mark failed
uv run python .claude/scripts/orchestrator.py checkpoint phase-X "msg"
uv run python .claude/scripts/orchestrator.py rollback   # Rollback
uv run python .claude/scripts/orchestrator.py reset      # Reset progress
```

### Hooks (Automatic)

| Event                    | Action                                  |
| ------------------------ | --------------------------------------- |
| PreToolUse (Write/Edit)  | Validate frontend for hardcoded strings |
| PreToolUse (Bash)        | Block dangerous commands                |
| PostToolUse (Write/Edit) | Run linting (Biome/Ruff)                |
| SessionStart             | Show migration context and progress     |

### Adding Tasks

Edit `orchestrator.py` TASKS dict - single source of truth for all task definitions, dependencies, and verification commands.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
