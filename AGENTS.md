# MFRC522 Async Driver - Development Guide

This file contains guidelines and commands for agentic coding agents working on
this MFRC522 async driver repository.

## Build, Test, and Lint Commands

### Essential Commands

```bash
# Check if code compiles (fastest)
cargo check

# Run all tests (unit + integration)
cargo test

# Run tests with output for debugging
cargo test -- --nocapture

# Run a specific test file
cargo test constructor_tests
cargo test reqa
cargo test crc

# Run a single specific test
cargo test test_new_with_empty_expectations -- --nocapture

# Lint with Clippy (recommended before commits)
cargo clippy -- -D warnings

# Format code with rustfmt
cargo fmt

# Check documentation
cargo doc --no-deps

# Run with specific features
cargo test --features defmt
```

### Development Workflow

1. Always run `cargo check` first to ensure compilation
2. Run `cargo clippy` to catch potential issues
3. Run `cargo test` to verify existing functionality
4. Use `cargo fmt` before committing

## Code Style Guidelines

### General Style

- **Edition**: Rust 2024
- **No unsafe code**: `#![deny(unsafe_code)]` enforced
- **Documentation**: All public items must have docs `#![deny(missing_docs)]`
- **No std**: `#![no_std]` library for embedded systems
- **Async-first**: All public API methods use `async fn`

### Imports and Dependencies

```rust
// Standard library imports - use embassy-time for async timing
use embassy_time::{Duration, WithTimeout};

// Embedded HAL traits - keep organized by category
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::{Operation, SpiDevice};

// Local modules - alphabetical order
mod error;
mod picc;
mod register;
mod util;

// Re-exports - group by functionality
pub use error::Error;
pub use picc::{Command, Sak, Type};
pub use register::{/* specific registers */};
```

### Error Handling

- Use the `Error<SpiE, GpioE>` enum for all public methods
- Map underlying SPI errors with `Error::Comm`
- Map GPIO errors with `Error::Gpio`
- Handle all 13 error variants in tests:
  - Bcc, BufferOverflow, Collision, Crc
  - IncompleteFrame, Overheating, Parity, Protocol
  - Timeout, Wr, Nak, NoRoom, Proprietary

### Async Patterns

- All public API methods are async
- Use embassy-time `with_timeout` for timeout handling
- Implement IRQ-based completion for hardware operations
- Return `Result<T, Error<SpiE, GpioE>>` from async methods

### Naming Conventions

- **Types**: `PascalCase` (e.g., `Mfrc522`, `AtqA`, `GenericUid`)
- **Functions**: `snake_case` (e.g., `reqa()`, `select()`, `calculate_crc()`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `RX_IRQ`, `POWER_DOWN`)
- **Registers**: Follow MFRC522 datasheet naming (e.g., `CommandReg`,
  `FIFODataReg`)

### State Machine Pattern

- Use phantom types for state management:

```rust
pub enum Unknown {}   // Initial state - don't know hardware's EN pin state
pub enum Disabled {}   // Hardware is definitely powered off (EN pin low)
pub enum Uninitialized {}  // Hardware powered on, awaiting initialization
pub enum Initialized {}    // Ready for use

impl<SPI, IRQ, EN> Mfrc522<SPI, IRQ, EN, Unknown> {
    pub fn new(spi: SPI, irq: IRQ, enable: EN) -> Self { /* ... */ }
    pub fn enable(self) -> Result<Mfrc522<SPI, IRQ, EN, Uninitialized>, EnableError<...>> { /* ... */ }
    pub fn disable(self) -> Result<Mfrc522<SPI, IRQ, EN, Disabled>, EnableError<...>> { /* ... */ }
    pub fn check_state(self) -> Result<PowerState<...>, EnableError<...>> { /* ... */ }
}

impl<SPI, IRQ, EN> Mfrc522<SPI, IRQ, EN, Disabled> {
    pub fn enable(self) -> Result<Mfrc522<SPI, IRQ, EN, Uninitialized>, EnableError<...>> { /* ... */ }
}

impl<SPI, IRQ, EN> Mfrc522<SPI, IRQ, EN, Uninitialized> {
    pub async fn init(self) -> Result<Mfrc522<SPI, IRQ, EN, Initialized>, Error<_, _>> { /* ... */ }
    pub fn disable(self) -> Result<Mfrc522<SPI, IRQ, EN, Disabled>, EnableError<...>> { /* ... */ }
}

impl<SPI, IRQ, EN> Mfrc522<SPI, IRQ, EN, Initialized> {
    pub async fn reqa(&mut self) -> Result<AtqA, Error<_, _>> { /* ... */ }
    pub fn disable(self) -> Result<Mfrc522<SPI, IRQ, EN, Disabled>, EnableError<...>> { /* ... */ }
}

// PowerState enum for check_state()
pub enum PowerState<SPI, IRQ, EN> {
    Enabled(Mfrc522<SPI, IRQ, EN, Uninitialized>),
    Disabled(Mfrc522<SPI, IRQ, EN, Disabled>),
}
```

## Testing Guidelines

### Updated Bisection Debugging Approach

**Critical Discovery**: When SPI transaction tests fail, use this systematic
bisection approach:

1. **Insert `SpiTransaction::flush()` at the failure point** to find the exact
   command causing issues
2. **Use bisection to narrow down the problem**:
   ```rust
   // Start with flushing after half of transactions
   let expectations = [
       // ... first half of transactions
       SpiTransaction::Flush, // Insert here
       // ... rest of transactions
   ];
   ```
3. **Move the Flush** based on results:
   - If test passes → issue is after the Flush
   - If test fails → issue is before the Flush
4. **Repeat until** you isolate the exact problematic transaction
5. **Verify the exact bytes** being sent vs expected

**Error Message Interpretation:**

- `left: [2, 0] right: [8, 127]` means mock received `[2, 0]` but expected
  `[8, 127]`
- `left` = actual values passed during test execution
- `right` = values in expectation array
- Use this to identify which specific transaction has wrong bytes/values

**Key Finding**: The async implementation requires **all register writes to be
wrapped in `transaction_start/end`** because
`embedded_hal_async::SpiDevice::write()` internally uses transactions.

### SPI Mock Usage

Use `embedded-hal-mock` for testing SPI interactions:

```rust
use embedded_hal_mock::eh1::digital::Mock as PinMock;
use embedded_hal_mock::eh1::spi::Mock as SpiMock;

#[tokio::test]
async fn test_something() {
    let mut spi = SpiMock::new(&[]);
    let mut irq = PinMock::new(&[]);
    let mut enable = PinMock::new(&[]);
    
    // Test code here
    
    // Always verify mocks
    spi.done();
    irq.done();
    enable.done();
}
```

### Critical: SPI Transaction Debugging with Bisection

**When SPI transaction tests fail, use this systematic approach:**

1. **Insert `SpiTransaction::Flush` at the failure point** to find the exact
   command causing issues
2. **Use bisection to narrow down the problem**:
   ```rust
   // Start with flushing after half the transactions
   let expectations = [
       // ... first half of transactions
       SpiTransaction::Flush, // Insert here
       // ... rest of transactions
   ];
   ```
3. **Move the Flush** based on results:
   - If test passes → issue is after the Flush
   - If test fails → issue is before the Flush
4. **Repeat until** you isolate the exact problematic transaction
5. **Verify the exact bytes** being sent vs expected

### Test Structure

```rust
//! Module documentation explaining what's being tested

use embedded_hal_mock::eh1::digital::Mock as PinMock;
use embedded_hal_mock::eh1::spi::Mock as SpiMock;
use mfrc522_async::Mfrc522;

#[tokio::test]
async fn test_specific_functionality() {
    // Arrange: Set up mocks with expected transactions
    let mut spi = SpiMock::new(&[
        // Define exact SPI transaction sequence
        SpiTransaction::write([register, data].to_vec()),
        SpiTransaction::transfer([expected_write].to_vec(), [expected_read].to_vec()),
    ]);
    let mut irq = PinMock::new(&[]);
    let mut enable = PinMock::new(&[]);
    
    // Act: Call the method under test
    let mut mfrc522 = Mfrc522::new(&mut spi, &mut irq, &mut enable);
    let result = mfrc522.some_method().await;
    
    // Assert: Verify results
    assert!(result.is_ok());
    
    // Verify all expectations were met
    spi.done();
    irq.done();
    enable.done();
}
```

### Mock Error Injection

Test error paths by configuring mocks to return errors:

```rust
// For SPI errors
let spi = SpiMock::new(&[
    SpiTransaction::write([0x02, 0x00].to_vec()).with_error(SpiError::Other),
]);

// For GPIO/IRQ errors (if needed)
let irq = PinMock::new(&[]).with_error(GpioError::Other);
```

### Test Organization

- Unit tests in `src/lib.rs` for internal data structures
- Integration tests in `tests/` directory:
  - `constructor_tests.rs` - Basic construction
  - `init.rs` - Initialization sequence
  - `reqa.rs` - REQA command testing
  - `crc.rs` - CRC calculation testing
  - `transceive.rs` - Low-level communication testing

### Common Test Patterns

1. **Happy Path**: Normal successful operation
2. **Error Paths**: All error variants should be tested
3. **Edge Cases**: Boundary conditions, empty/full buffers
4. **Timeout Scenarios**: IRQ never triggers
5. **Protocol Violations**: Invalid responses, CRC failures

### Async Test Requirements

- All tests must be `#[tokio::test]`
- Use tokio runtime for async execution
- Test async behavior with proper timeout handling

## Project Structure

### Key Files

- `src/lib.rs` - Main library implementation
- `src/error.rs` - Error type definitions
- `src/register.rs` - MFRC522 register constants and commands
- `src/picc.rs` - PICC-related enums and types
- `src/util.rs` - Utility traits and helpers
- `tests/` - Integration test modules

### Dependencies

- **Core**: `embedded-hal-async`, `embassy-time`, `embassy-futures`
- **Dev**: `embedded-hal-mock`, `tokio`, `futures`
- **Optional**: `defmt` for embedded debugging

## Development Priorities

1. **Async Patterns**: Maintain consistency in async error handling
2. **Register Sequences**: Follow MFRC522 datasheet for exact register
   operations
3. **Type Safety**: Leverage Rust's type system for state management
4. **Error Handling**: Ensure all error paths are properly handled and tested

## Hardware Protocol Notes

- **SPI Mode**: MFRC522 uses specific SPI addressing (read: 0x80, write: 0x00)
- **IRQ Handling**: Use interrupt-driven completion for non-blocking operations
- **Register Access**: Always verify register sequences against MFRC522
  datasheet
- **CRC Calculation**: Hardware-accelerated CRC must be properly configured
- **Antenna Gain**: Adjust `RxGain` for communication reliability issues

Remember: This is a no_std embedded library. Keep memory usage minimal and avoid
dynamic allocation.

