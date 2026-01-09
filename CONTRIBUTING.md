# Contributing to Nexus Audio Player

Thank you for your interest in contributing to Nexus Audio Player! 🎵

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Coding Standards](#coding-standards)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for everyone.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/your-username/nexus-monorepo.git
   cd nexus-monorepo
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/original-owner/nexus-monorepo.git
   ```

## Development Setup

### Prerequisites

- Rust 1.70+
- Node.js 18+
- npm or pnpm
- Tauri system dependencies (see [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites))

### Installation

1. **Install frontend dependencies**:
   ```bash
   cd nexus-player-frontend
   npm install
   ```

2. **Build the backend**:
   ```bash
   cd ../nexus-player-backend
   cargo build
   ```

3. **Run development server**:
   ```bash
   cd ../nexus-player-frontend
   npm run start
   ```

## How to Contribute

### Reporting Bugs

Before creating a bug report:
- Check the issue tracker to avoid duplicates
- Collect information about the bug (OS, versions, steps to reproduce)

Create an issue with:
- Clear, descriptive title
- Detailed steps to reproduce
- Expected vs actual behavior
- Screenshots if applicable
- Environment details

### Suggesting Features

Feature suggestions are welcome! Please:
- Use a clear, descriptive title
- Provide detailed explanation of the feature
- Explain why this feature would be useful
- Include mockups or examples if possible

### Code Contributions

1. **Pick an issue** or create one to discuss your changes
2. **Create a branch**:
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/your-bug-fix
   ```

3. **Make your changes** following our [coding standards](#coding-standards)

4. **Test your changes**:
   ```bash
   # Backend tests
   cd nexus-player-backend
   cargo test

   # Frontend build
   cd ../nexus-player-frontend
   npm run build
   ```

5. **Commit your changes** following our [commit guidelines](#commit-guidelines)

6. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

7. **Create a Pull Request** from your fork to our `master` branch

## Coding Standards

### Rust (Backend)

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` to catch common mistakes
- Write meaningful comments for complex logic
- Add unit tests for new functionality

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

### TypeScript/Vue (Frontend)

- Follow Vue 3 best practices with Composition API
- Use TypeScript for type safety
- Use meaningful component and variable names
- Keep components focused and reusable
- Add JSDoc comments for complex functions

```bash
npm run build  # Ensures TypeScript compilation succeeds
```

### General Guidelines

- Keep functions small and focused
- Avoid deep nesting
- Use descriptive variable and function names
- Don't commit commented-out code
- Remove console.log statements before committing

## Commit Guidelines

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

### Examples

```
feat(player): add volume control slider
fix(bandcamp): resolve HLS streaming bug
docs(readme): update installation instructions
refactor(backend): simplify audio decoder logic
```

## Pull Request Process

1. **Update documentation** if you changed APIs or added features
2. **Add tests** for new functionality
3. **Ensure all tests pass** and code builds successfully
4. **Update the README.md** if needed
5. **Reference related issues** in your PR description
6. **Wait for review** - maintainers will review your PR

### PR Title Format

Use the same format as commit messages:
```
feat(scope): add new feature
fix(scope): fix the bug
```

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
How did you test these changes?

## Related Issues
Fixes #(issue number)
```

## Questions?

Feel free to:
- Open an issue for discussion
- Ask questions in existing issues
- Reach out to maintainers

---

Thank you for contributing! 🎉
