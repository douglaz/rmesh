# Rust Coding Conventions

## String Interpolation
For format!, println!, info!, debug!, and similar macros:

### Correct Usage:
- ALWAYS use direct variable names when they match the placeholder name:
  ```rust
  let name = "John";
  println!("Hello {name}");  // GOOD - Direct use of variable name in placeholder

  // This also applies to all logging macros
  let endpoint = "users";
  debug!("Processing request for {endpoint}");  // GOOD
  ```

- ONLY use named parameters when using a property or method:
  ```rust
  println!("Count: {count}", count = items.len());  // GOOD - Method call needs named parameter
  ```

- ALWAYS use placeholder names that match the variable names. NEVER create temporary variables just to match placeholder names:
  ```rust
  // GOOD - Placeholder name matches variable name
  println!("Checked {files_checked} files");

  // GOOD - Named parameter for method call
  println!("Found {errors_count} errors", errors_count = errors.len());

  // BAD - Don't create temporary variables to match placeholders
  let files = files_checked; // DON'T do this
  let errors = errors.len(); // DON'T do this
  println!("Checked {files} files, found {errors} errors");
  ```

### Format Specifiers:
- Use format specifiers explicitly when needed:
  ```rust
  // Debug format - use {variable:?} for Debug trait
  let items = vec![1, 2, 3];
  println!("Items: {items:?}");  // GOOD - Explicit debug format
  
  // Display format - use {variable} for Display trait (default)
  let name = "John";
  println!("Name: {name}");  // GOOD - Display format (implicit)
  
  // For durations and other types that need Debug
  let duration = std::time::Duration::from_secs(5);
  info!("Completed in {duration:?}");  // GOOD - Duration needs Debug format
  ```

### Incorrect Usage:
- Don't use positional arguments:
  ```rust
  let name = "John";
  println!("Hello {}", name);  // BAD - No named placeholder
  
  // Also BAD for debug formatting:
  let items = vec![1, 2, 3];
  println!("Items: {:?}", items);  // BAD - Use {items:?} instead
  ```

- Don't use redundant named parameters when the variable name matches:
  ```rust
  let name = "John";
  println!("Hello {name}", name = name);  // BAD - Redundant, just use "{name}"
  ```

- Don't use different names unnecessarily:
  ```rust
  let name = "John";
  println!("Hello {user}", user = name);  // BAD - Not property or method, just use "{name}" directly
  ```

### Exceptions:
Display trait implementations are an exception to the named placeholder rule:
  ```rust
  // CORRECT - Display implementations use positional arguments by convention
  impl fmt::Display for MyType {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
          write!(f, "{}", self.0)  // OK - This is the idiomatic way for Display impl
      }
  }
  ```

## Error Handling

### Correct Usage:
- ALWAYS use anyhow for error handling, particularly bail! and ensure!:
  ```rust
  // For conditional checks
  ensure!(condition, "Error message with {value}");

  // For early returns with errors
  bail!("Failed with error: {error_message}");
  ```

- IMPORTANT: When using `.context()` vs `.with_context()` for error handling:
  ```rust
  // For static error messages with no variables:
  let result = some_operation.context("Operation failed")?;

  // For error messages with variables - ALWAYS use with_context with a closure and format!:
  let id = "123";

  // GOOD - Direct variable name in placeholder for simple variables
  let result = some_operation
      .with_context(|| format!("Failed to process item {id}"))?;

  // GOOD - Named parameter for property or method calls
  let path = std::path::Path::new("file.txt");
  let result = std::fs::read_to_string(path)
      .with_context(|| format!("Failed to read file: {path}", path = path.display()))?;

  // BAD - Don't use positional parameters
  // .with_context(|| format!("Failed to read file: {}", path.display()))?

  // NEVER use context() with variables directly in the string - this won't work:
  // BAD: .context("Failed to process item {id}")  // variables won't interpolate!

  // NEVER use context() with format!() - this is inneficient!:
  // BAD: .context(format!("Failed to process item {id}"))? // use .with_context(|| format!(...))
  ```

- REMEMBER: All string interpolation rules apply to ALL format strings, including those inside `with_context` closures

### Incorrect Usage:
- NEVER use unwrap() or panic!:
  ```rust
  // BAD - Will crash on None:
  let result = optional_value.unwrap();

  // BAD - Will crash on Err:
  let data = fallible_operation().unwrap();

  // BAD - Explicit panic:
  panic!("This failed");
  ```

- Avoid using .ok() or .expect() to silently ignore errors:
  ```rust
  // BAD - Silently ignores errors:
  std::fs::remove_file(path).ok();

  // BETTER - Log the error but continue:
  if let Err(e) = std::fs::remove_file(path) {
      debug!("Failed to remove file: {e}");
  }
  ```

## Code Quality Standards

### Finding Convention Violations

Use these commands to find potential violations of string interpolation conventions:

```bash
# Find positional placeholders (excluding Display trait implementations)
rg -F '{}' | grep -v 'write!(f,' | grep -v '// ' | grep -v '# '

# Find debug positional placeholders
rg -F '{:?}'

# Find any positional placeholders with formatting
rg '\{:[^}]+\}' --pcre2

# Find format strings with positional arguments (more comprehensive)
rg 'format!\([^)]*\{[^a-zA-Z_]' --pcre2

# Find print/log macros with positional arguments
rg '(println!|print!|eprintln!|eprint!|info!|debug!|warn!|error!|trace!)\([^)]*"[^"]*\{\}' --pcre2

# Find .context() with format! (should use .with_context() instead)
rg '\.context\(format!'

# Find error messages with positional placeholders in bail! or ensure!
rg '(bail!|ensure!)\([^)]*"[^"]*\{\}' --pcre2
```

**Note**: When reviewing results, remember these exceptions are acceptable:
- Display trait implementations (`write!(f, "{}", self.0)`)
- JSON literals (`serde_json::json!({})`)
- Documentation examples and comments
- Match arms with empty blocks (`=> {}`)
- Struct initialization with no fields

### Always Run After Significant Changes:
1. **Format code** - Ensures consistent code style:
   ```bash
   cargo fmt --all
   ```

2. **Run clippy** - Catches common mistakes and suggests improvements:
   ```bash
   cargo clippy --locked --offline --workspace --all-targets -- --deny warnings
   ```

3. **Run tests** - Ensures no regressions:
   ```bash
   cargo test
   ```

### When to Run These Commands:
- After implementing a new feature
- After refactoring existing code
- Before creating a pull request
- After resolving merge conflicts
- After any changes that touch multiple files

**Important**: Code must be properly formatted and pass all clippy checks before being committed to the repository.
