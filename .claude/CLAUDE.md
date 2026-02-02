# general

1. When working in an existing project, first inspect and follow that project's established design patterns and architecture. If those patterns conflict with these global rules, ask which pattern to follow for that project before changing code.
2. When refactoring code make sure to tidy up after yourslef and look at old references and implementations to understand the context anbd tidy up old logic that is either no longer used and/or no longer needed/outdated and needs updating to the new pattern.
3. The local machine is MacosX with zsh as default shell.
4. The local machine cpu arch is ARM (M4 max chip).
5. for building an ddeploying docker containers for remote purposes we most likely should use x86 build by default.
6. I am on macos and use zsh so make sure cli commands work for this. also ensure that usally remote is some linux with bash as default shell so things need to also work there.
7. try use ripgrep for searching and fallback to grep. again locally i am on macos so grep is different from gnu grep.
8. Always use context7 when I need code generation, setup or configuration steps, or library/API documentation. This means you should automatically use the Context7 MCP tools to resolve library id and get library docs without me having to explicitly ask.
9. Do NOT be syccophantic. Be bold and ask questions if you are not sure about something. Show scrutiny and think critically
10.check if a logger is configured and use that over print statements and console log
11. always aim for reusable logic and compoenents. check if similar functionality exists and if so refactor into shared re-usabel logic pieces of code.
12. rememebr to check linting, formatting and running tests after making changes to code and wantitng to commit chnages.
13. check the code base for makefile taskfile or just file. use recipes from there. extend if necessary 

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

# typescript

1. prefer const style over functions for functions
2. Use turbo repo and monorepo structure based on turborepo as a preference
3. prefer interfaces and implements over classes
4. use bun for running scripts
5. for React prefer const arrow functional components as opposed to classes or functions
6. use bun for formatting via bun biome and biome.config.js
7. use bun for linting via bun biome and biome.config.js
8. use bun for testing via bun jest and jest.config.js
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
- ✅ Make every condition easy to read and understand%     
