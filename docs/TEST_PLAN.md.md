# Test Plan - LightMon

## 1. Introduction
This document describes the testing strategy for **LightMon**, a lightweight system monitor built in Rust using the `iced` GUI framework. The goal of testing is to ensure that the application:

- Accurately reports system metrics (CPU, memory, disk usage).  
- Correctly lists and filters running processes.  
- Supports theme switching and settings updates.  
- Handles configuration save/load operations reliably.  
- Exports process data to CSV successfully.  

## 2. Scope
The tests cover:

- Functional testing of each feature.  
- Integration testing of components (system data, GUI, settings).  
- User interface validation.  
- Configuration file read/write operations.  
- CSV export functionality.  

**Excluded from testing:**

- Low-level OS-specific system data inaccuracies (depends on `sysinfo`).  
- Performance benchmarks.  

## 3. Test Environment
- **OS:** Windows 10 / Linux / macOS  
- **Rust version:** 1.70+  
- **Dependencies:** `iced`, `sysinfo`, `serde`, `toml`, `tokio`  
- **Terminal / Console**: For running test cases (`cargo test`)  
- **Tools:** `cargo`, text editor / IDE  

## 4. Test Cases

### 4.1 Functional Test Cases

| ID    | Test Description         | Steps                           | Expected Result |
|-------|-------------------------|---------------------------------|----------------|
| TC-01 | Application startup     | `cargo run --release`           | Main window opens with Overview tab showing CPU, memory, and disk stats. |
| TC-02 | Theme toggle            | Click theme button              | Theme switches between light and dark, config updated. |
| TC-03 | Switch tabs             | Click Overview / Processes / Settings | Correct screen displayed. |
| TC-04 | Refresh interval change | Input valid number in Settings  | Application updates interval, config saved. |
| TC-05 | Invalid refresh input   | Input non-numeric string        | Refresh interval remains unchanged, input field updates text. |
| TC-06 | Process filter          | Type in search box              | List filters by name or PID. |
| TC-07 | Sort processes          | Click CPU/Memory button         | Processes sorted accordingly. |
| TC-08 | Export to CSV           | Click Export button             | File `processes.csv` created with correct data. |

### 4.2 Integration Test Cases

| ID    | Test Description       | Steps                        | Expected Result |
|-------|-----------------------|------------------------------|----------------|
| IT-01 | Config persistence     | Change settings, restart app | Settings retained from previous session. |
| IT-02 | Process selection      | Click on a process           | Detailed information displayed below process list. |
| IT-03 | Toast messages         | Trigger export or error      | Toast appears with appropriate message, disappears after 3 seconds. |

### 4.3 Unit Tests
- Verify `fetch_system_data` returns valid values.  
- Verify `AppConfig::default()` values.  
- Verify sorting and filtering functions.  
- Verify CSV export creates file with proper content.  

### 4.4 Error Handling Tests
- Test permission errors when writing CSV or config file.  
- Test empty process list handling.  
- Test invalid config file format.  

## 5. Test Schedule
- Unit tests: Ongoing during development (`cargo test`)  
- Functional & integration tests: After feature completion  
- Regression tests: Before final release  

## 6. Test Deliverables
- Test cases table (this document)  
- Screenshots/logs of test results  
- Tested CSV files  
- Test summary report  

## 7. Approval
| Name          | Role               | Signature | Date |
|---------------|------------------|-----------|------|
| Ye Khaung Soe | Developer / Tester |  ye     |  03/11/2025    |