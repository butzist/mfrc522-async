# MFRC522 Async Library - Comprehensive Test Plan

## Overview

This document outlines the complete test coverage needed for the MFRC522 async
library to ensure production readiness. Current tests only cover basic data
structures (7 unit tests) with **0% coverage** of public API functionality.

## Current Test Coverage Analysis

### ✅ What's Currently Covered

- Basic data structure creation (AtqA, Uid, GenericUid, FifoData)
- PICC command enum values
- Register address constants
- SAK type detection logic
- **Total: 7 unit tests, all synchronous**

### ❌ Critical Missing Coverage

- **0%** of public async API methods (`init()`, `reqa()`, `select()`)
- **0%** error handling variants (13 error types, none tested)
- **0%** integration/async testing
- **0%** edge cases and boundary conditions
- **0%** protocol compliance testing

---

## Phase 1: Core Infrastructure Tests

### 1.1 Mock Implementation Setup

**Priority: CRITICAL** **Goal: Use async mock implementations for
embedded-hal-async traits**

Use embedded-hal-mock for the following tests - check that it is compatible with
the constructor

### 1.2 Constructor Tests

**Priority: HIGH** **Files:** `tests/constructor_tests.rs`

```rust
#[cfg(test)]
mod constructor_tests {
    // Test 1.1: Basic construction
    #[tokio::test]
    async fn test_new_basic() {
        let spi = MockSpiDevice::new();
        let irq = MockIrqPin::new();
        let mfrc522 = Mfrc522::new(spi, irq);
        // Verify internal state
    }
}
```

---

## Phase 2: Public API Tests

### 2.1 Initialization Tests (`init()`)

**Priority: CRITICAL** **Files:** `tests/init_tests.rs`

```rust
// Test 2.1.1: Successful initialization
#[tokio::test]
async fn test_init_success() {
    // Mock successful register writes
    // Verify all init sequence steps
}

// Test 2.1.2: SPI communication failure during init
#[tokio::test]
async fn test_init_spi_failure() {
    // Mock SPI error during register write
    // Verify proper error propagation
}

// Test 2.1.3: Power-down detection timeout
#[tokio::test]
async fn test_init_power_down_timeout() {
    // Mock power-down bit never clearing
    // Verify timeout error
}
```

### 2.2 REQA Tests (`reqa()`)

**Priority: CRITICAL** **Files:** `tests/reqa_tests.rs`

```rust
// Test 2.2.1: Successful REQA with ATQA response
#[tokio::test]
async fn test_reqa_success() {
    // Mock complete REQA transaction
    // Verify ATQA parsing
}

// Test 2.2.2: REQA timeout
#[tokio::test]
async fn test_reqa_timeout() {
    // Mock IRQ never triggers
    // Verify timeout error
}

// Test 2.2.3: REQA collision detected
#[tokio::test] 
async fn test_reqa_collision() {
    // Mock collision during REQA
    // Verify collision error
}

// Test 2.2.4: REQA protocol error
#[tokio::test]
async fn test_reqa_protocol_error() {
    // Mock protocol error in response
    // Verify protocol error handling
}
```

### 2.3 SELECT Tests (`select()`)

**Priority: HIGH** **Files:** `tests/select_tests.rs`

```rust
// Test 2.3.1: Single UID selection (4-byte)
#[tokio::test]
async fn test_select_single_uid() {
    // Mock successful anticollision + select
    // Verify UID extraction and SAK parsing
}

// Test 2.3.2: Double UID selection (7-byte, cascade)
#[tokio::test]
async fn test_select_double_uid() {
    // Mock cascade level 2 selection
    // Verify proper cascade handling
}

// Test 2.3.3: Triple UID selection (10-byte, cascade)
#[tokio::test]
async fn test_select_triple_uid() {
    // Mock cascade level 3 selection
    // Verify complete cascade sequence
}

// Test 2.3.4: Selection collision during anticollision
#[tokio::test]
async fn test_select_anticollision_collision() {
    // Mock collision during anticollision phase
    // Verify collision error and retry logic
}

// Test 2.3.5: Selection CRC validation failure
#[tokio::test]
async fn test_select_crc_validation() {
    // Mock CRC mismatch in selection
    // Verify CRC error handling
}

// Test 2.3.6: Proprietary anticollision protocol
#[tokio::test]
async fn test_select_proprietary_protocol() {
    // Mock ATQA indicating proprietary protocol
    // Verify proprietary error handling
}
```

### 2.4 CRC Calculation Tests (`calculate_crc()`)

**Priority: MEDIUM** **Files:** `tests/crc_tests.rs`

```rust
// Test 2.4.1: Standard CRC calculation
#[tokio::test]
async fn test_calculate_crc_standard() {
    // Test known CRC values against implementation
    // Verify against reference implementations
}

// Test 2.4.2: Empty data CRC
#[tokio::test]
async fn test_calculate_crc_empty() {
    // Test CRC calculation with no data
    // Verify expected result
}

// Test 2.4.3: Large data CRC
#[tokio::test]
async fn test_calculate_crc_large_data() {
    // Test CRC with maximum FIFO size
    // Verify performance and correctness
}
```

---

## Phase 3: Error Handling Tests

### 3.1 Communication Error Tests

**Priority: HIGH** **Files:** `tests/error_tests.rs`

```rust
// Test 3.1.1: SPI write failures
#[tokio::test]
async fn test_error_spi_write_failure() {
    // Mock SPI write error during register access
    // Verify Error::Comm propagation
}

// Test 3.1.2: SPI read failures
#[tokio::test]
async fn test_error_spi_read_failure() {
    // Mock SPI read error during register access
    // Verify Error::Comm propagation
}

// Test 3.1.3: FIFO buffer overflow
#[tokio::test]
async fn test_error_fifo_overflow() {
    // Mock FIFO overflow condition
    // Verify Error::BufferOverflow
}

// Test 3.1.4: CRC validation errors
#[tokio::test]
async fn test_error_crc_validation() {
    // Mock CRC validation failure
    // Verify Error::Crc
}
```

### 3.2 Protocol Error Tests

```rust
// Test 3.2.1: Collision detection errors
#[tokio::test]
async fn test_error_collision_detection() {
    // Mock collision register set
    // Verify Error::Collision
}

// Test 3.2.2: Protocol violation errors
#[tokio::test]
async fn test_error_protocol_violation() {
    // Mock protocol error in response
    // Verify Error::Protocol
}

// Test 3.2.3: Incomplete frame errors
#[tokio::test]
async fn test_error_incomplete_frame() {
    // Mock incomplete frame response
    // Verify Error::IncompleteFrame
}

// Test 3.2.4: BCC (Block Check Character) errors
#[tokio::test]
async fn test_error_bcc_mismatch() {
    // Mock BCC validation failure
    // Verify Error::Bcc
}
```

### 3.3 System Error Tests

```rust
// Test 3.3.1: Overheating detection
#[tokio::test]
async fn test_error_overheating() {
    // Mock temperature error flag
    // Verify Error::Overheating
}

// Test 3.3.2: Parity errors
#[tokio::test]
async fn test_error_parity_failure() {
    // Mock parity error flag
    // Verify Error::Parity
}

// Test 3.3.3: Write timing errors
#[tokio::test]
async fn test_error_write_timing() {
    // Mock write timing violation
    // Verify Error::Wr
}

// Test 3.3.4: NAK (Not Acknowledge) errors
#[tokio::test]
async fn test_error_nak_response() {
    // Mock NAK response from PICC
    // Verify Error::Nak
}
```

---

## Phase 4: Edge Cases and Boundary Conditions

### 4.1 UID Handling Edge Cases

**Priority: MEDIUM** **Files:** `tests/edge_case_tests.rs`

```rust
// Test 4.1.1: Minimum valid UID (1 byte)
#[tokio::test]
async fn test_uid_minimum_size() {
    // Test edge case with minimal UID
}

// Test 4.1.2: Maximum cascade levels
#[tokio::test]
async fn test_uid_maximum_cascade() {
    // Test with maximum allowed cascade
}

// Test 4.1.3: Malformed UID data
#[tokio::test]
async fn test_uid_malformed_data() {
    // Test handling of invalid UID patterns
}
```

### 4.2 FIFO Operations Edge Cases

```rust
// Test 4.2.1: FIFO empty conditions
#[tokio::test]
async fn test_fifo_empty_operations() {
    // Test operations on empty FIFO
}

// Test 4.2.2: FIFO full conditions  
#[tokio::test]
async fn test_fifo_full_operations() {
    // Test operations on full FIFO
}

// Test 4.2.3: Partial byte handling
#[tokio::test]
async fn test_fifo_partial_bytes() {
    // Test valid_bits < 8 scenarios
}

// Test 4.2.4: Bit-level boundary conditions
#[tokio::test]
async fn test_fifo_bit_boundaries() {
    // Test copy_bits_to edge cases
}
```

---

## Phase 5: Integration Tests

### 5.1 Complete Workflow Tests

**Priority: HIGH** **Files:** `tests/integration_tests.rs`

```rust
// Test 5.1.1: Full card detection workflow
#[tokio::test]
async fn test_integration_complete_workflow() {
    // Mock: init -> reqa -> select -> validate
    // Test entire happy path
}

// Test 5.1.2: Multi-card detection scenarios
#[tokio::test]
async fn test_integration_multiple_cards() {
    // Simulate multiple cards in field
    // Test collision handling and resolution
}

// Test 5.1.3: Card removal scenarios
#[tokio::test]
async fn test_integration_card_removal() {
    // Test behavior when card disappears mid-operation
    // Verify timeout and error handling
}
```

## Implementation Priority Matrix

| Phase             | Priority | Estimated Tests | Key Focus            |
| ----------------- | -------- | --------------- | -------------------- |
| 1: Infrastructure | CRITICAL | 5-10            | Mock implementations |
| 2: Public API     | CRITICAL | 15-25           | All public methods   |
| 3: Error Handling | HIGH     | 15-20           | All error variants   |
| 4: Edge Cases     | MEDIUM   | 10-15           | Boundary conditions  |
| 5: Integration    | HIGH     | 8-12            | Real workflows       |

**Total Estimated Tests: 59-92**

---

## Success Criteria

### Phase 1 Success

- [ ] Mock SPI implements embedded_hal_async::SpiDevice correctly
- [ ] Mock IRQ implements embedded_hal_async::digital::Wait correctly
- [ ] Constructor creates valid driver instances
- [ ] Mocks support error injection capabilities

### Phase 2 Success

- [ ] All public API methods have async unit test coverage
- [ ] Each method tests success paths and all error conditions
- [ ] Tests verify proper register sequences
- [ ] Tests validate data parsing and formatting

### Phase 3 Success

- [ ] All 13 Error<E> variants have test coverage
- [ ] Error propagation verified from low to high level
- [ ] Error recovery scenarios tested
- [ ] Error messages and debugging info verified

### Phase 4 Success

- [ ] All boundary conditions identified and tested
- [ ] Edge case behavior documented
- [ ] Invalid input handling verified
- [ ] Resource limit conditions tested

### Phase 5 Success

- [ ] Complete integration workflows tested
- [ ] Performance characteristics measured
- [ ] Memory usage verified in stress conditions
- [ ] Platform independence demonstrated

---

## Implementation Notes

### File Organization

```
src/
├── lib.rs                    # Main library + existing tests
├── tests/
│   ├── mod.rs              # Test module declaration
│   ├── mocks.rs             # Mock implementations
│   ├── constructor_tests.rs   # Constructor tests
│   ├── init_tests.rs         # Initialization tests
│   ├── reqa_tests.rs         # REQA command tests
│   ├── select_tests.rs       # SELECT command tests
│   ├── crc_tests.rs          # CRC calculation tests
│   ├── error_tests.rs        # Error handling tests
│   ├── edge_case_tests.rs    # Edge case tests
│   ├── integration_tests.rs   # Integration tests
└── ...
```

---

## Conclusion

This comprehensive test plan will transform the MFRC522 async library from
**prototype/demo status** to **production-ready** status by providing:

1. **Complete public API test coverage** - All async methods tested
2. **Robust error handling verification** - All 13 error variants covered
3. **Integration confidence** - Real-world workflow validation
4. **Edge case reliability** - Boundary condition protection
5. **Protocol compliance assurance** - ISO 14443A standard adherence

**Implementation Priority:** Start with Phase 1 (Infrastructure) → Phase 2
(Public API) → Phase 3 (Error Handling) → Phase 5 (Integration) for maximum
impact.

This plan provides a clear roadmap for achieving comprehensive test coverage and
production readiness for the MFRC522 async library.
