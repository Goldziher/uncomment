use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the compiled uncomment binary
fn get_binary_path() -> std::path::PathBuf {
    let build_output = Command::new("cargo")
        .args(["build", "--bin", "uncomment"])
        .output()
        .expect("Failed to build binary");

    if !build_output.status.success() {
        panic!(
            "Failed to build binary: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );
    }

    let mut binary_path = std::env::current_dir().expect("Failed to get current directory");
    binary_path.push("target/debug/uncomment");
    binary_path
}

/// Comprehensive integration test for the init command
/// Tests the actual CLI binary to ensure everything works end-to-end
#[test]
fn test_init_command_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    create_test_project(project_dir);

    test_smart_init(project_dir);

    test_comprehensive_init(project_dir);

    test_generated_config_processing(project_dir);

    test_force_overwrite(project_dir);
}

fn create_test_project(project_dir: &std::path::Path) {
    fs::create_dir_all(project_dir.join("src")).unwrap();
    fs::create_dir_all(project_dir.join("frontend")).unwrap();
    fs::create_dir_all(project_dir.join("mobile")).unwrap();
    fs::create_dir_all(project_dir.join("scripts")).unwrap();
    fs::create_dir_all(project_dir.join("tests")).unwrap();

    fs::write(
        project_dir.join("src/main.rs"),
        r#"
// Main entry point
fn main() {
    // TODO: implement CLI
    println!("Hello, world!"); // Debug output
    /* Multi-line comment
       with details */
}

/// Documentation for add function
/// Returns the sum of two numbers
fn add(a: i32, b: i32) -> i32 {
    a + b // Simple addition
}
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("src/lib.rs"),
        r#"
//! Library documentation
//! This is the main library

/// Public API function
pub fn process_data(input: &str) -> String {
    // FIXME: improve error handling
    input.to_uppercase() // Simple transformation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_data() {
        // Test the function
        assert_eq!(process_data("hello"), "HELLO");
    }
}
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("frontend/app.js"),
        r#"
// Main application
/* eslint-disable no-console */
console.log("Starting app"); // Debug log

/**
 * Main app function
 * @param {string} config - Configuration
 */
function initApp(config) {
    // TODO: add error handling
    console.log("App initialized");
}

// Export for modules
export { initApp };
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("frontend/types.d.ts"),
        r#"
// Type definitions
/* @ts-ignore missing types */
interface AppConfig {
    // Configuration options
    debug: boolean; // Enable debug mode
    /** API endpoint URL */
    apiUrl: string;
}

// TODO: add more types
export { AppConfig };
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("frontend/component.tsx"),
        r#"
import React from 'react';

// React component
interface Props {
    // Component props
    title: string; // Display title
}

/**
 * Main component
 * @param props Component properties
 */
const App: React.FC<Props> = ({ title }) => {
    // TODO: add state management
    return (
        <div>
            {/* Main content */}
            <h1>{title}</h1> {/* Title display */}
        </div>
    );
};

export default App;
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("mobile/ContentView.swift"),
        r#"
import SwiftUI

// Main view
struct ContentView: View {
    // MARK: - Properties
    @State private var counter = 0 // Counter state

    var body: some View {
        VStack {
            // TODO: improve UI design
            Text("Counter: \(counter)") // Display counter
                .padding()

            /* Action button */
            Button("Increment") {
                counter += 1 // Increment action
            }
            .padding() // Add padding
        }
    }
}

/// Preview provider
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("scripts/build.py"),
        r#"
#!/usr/bin/env python3
"""
Build script for the project
"""

import os
import sys

def main():
    """Main build function"""
    # TODO: add proper error handling
    print("Building project...")  # Debug output

    # Check requirements
    if not os.path.exists("Cargo.toml"):  # pragma: no cover
        print("Error: Not a Rust project")  # Error message
        sys.exit(1)

    # Build command
    os.system("cargo build")  # Execute build

if __name__ == "__main__":
    main()  # Run main function
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("service.go"),
        r#"
package main

import (
    "fmt"
    "log"
)

// Service represents our main service
type Service struct {
    // Configuration
    name string // Service name
    port int    // Port number
}

/*
NewService creates a new service instance
*/
func NewService(name string, port int) *Service {
    // TODO: add validation
    return &Service{
        name: name, // Set name
        port: port, // Set port
    }
}

// Start begins the service
func (s *Service) Start() error {
    // FIXME: implement proper startup
    fmt.Printf("Starting %s on port %d\n", s.name, s.port) // Debug log
    return nil
}

func main() {
    // Create and start service
    svc := NewService("test-service", 8080) // Initialize
    if err := svc.Start(); err != nil {
        log.Fatal(err) // Handle error
    }
}
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("config.yaml"),
        r#"
# Application configuration
app:
  name: "test-app"  # Application name
  version: "1.0.0"  # Version number

# Database settings
database:
  host: "localhost"  # DB host
  port: 5432         # DB port
  # TODO: add SSL configuration
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("Dockerfile"),
        r#"
# Multi-stage build
FROM rust:1.70 as builder

# Set working directory
WORKDIR /app

# Copy source
COPY . .

# Build application
# TODO: optimize build layers
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/app /usr/local/bin/app

# Run application
CMD ["app"]
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("frontend/App.vue"),
        r#"
<template>
  <!-- Main app template -->
  <div id="app">
    <!-- Header section -->
    <header>
      <h1>{{ title }}</h1> <!-- Page title -->
    </header>
    <!-- TODO: add navigation -->
    <main>
      <router-view /> <!-- Router outlet -->
    </main>
  </div>
</template>

<script>
export default {
  name: 'App',
  data() {
    return {
      // Component data
      title: 'My App' // Default title
    }
  },
  // TODO: add lifecycle hooks
  mounted() {
    // Component mounted
    console.log('App mounted') // Debug log
  }
}
</script>

<style>
/* Global styles */
#app {
  font-family: Arial, sans-serif; /* Main font */
  /* TODO: improve styling */
}
</style>
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("tests/integration_test.py"),
        r#"
"""Integration tests for the application"""

import pytest
import requests

class TestAPI:
    """API integration tests"""

    def test_health_endpoint(self):
        """Test health check endpoint"""
        # TODO: make URL configurable
        response = requests.get("http://localhost:8080/health")  # Health check
        assert response.status_code == 200  # Should be OK

    def test_user_creation(self):
        """Test user creation endpoint"""
        # Test data
        user_data = {
            "name": "Test User",  # User name
            "email": "test@example.com"  # User email
        }

        # Make request
        response = requests.post("http://localhost:8080/users", json=user_data)  # Create user
        # FIXME: add proper assertions
        assert response.status_code in [200, 201]  # Success codes
"#,
    )
    .unwrap();

    fs::write(
        project_dir.join("Makefile"),
        r#"
# Project Makefile

# Default target
.PHONY: all
all: build test

# Build the project
.PHONY: build
build:
	# TODO: parallelize builds
	cargo build --release  # Build Rust code
	cd frontend && npm run build  # Build frontend

# Run tests
.PHONY: test
test:
	# Run all tests
	cargo test  # Rust tests
	cd frontend && npm test  # Frontend tests
	python -m pytest tests/  # Python tests

# Clean build artifacts
.PHONY: clean
clean:
	# Remove build outputs
	cargo clean  # Clean Rust builds
	rm -rf frontend/dist  # Remove frontend dist
"#,
    )
    .unwrap();
}

fn test_smart_init(project_dir: &std::path::Path) {
    println!("Testing smart init...");

    let binary_path = get_binary_path();

    let output = Command::new(&binary_path)
        .args(["init", "--output", "smart-config.toml"])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Smart init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_path = project_dir.join("smart-config.toml");
    assert!(config_path.exists(), "Smart config file was not created");

    let config_content = fs::read_to_string(&config_path).unwrap();
    println!("Generated smart config:\n{}", config_content);

    assert!(config_content.contains("[global]"));
    assert!(!config_content.contains("# Smart Uncomment Configuration"));
    assert!(config_content.contains("[languages.rust]"));
    assert!(config_content.contains("[languages.javascript]"));
    assert!(config_content.contains("[languages.typescript]"));
    assert!(config_content.contains("[languages.python]"));
    assert!(config_content.contains("[languages.go]"));
    assert!(config_content.contains("[languages.swift]"));
    assert!(config_content.contains("[languages.vue]"));

    assert!(config_content.contains("[languages.rust]"));
    assert!(config_content.contains("[languages.javascript]"));
    assert!(config_content.contains("[languages.typescript]"));
    assert!(config_content.contains("[languages.python]"));
    assert!(config_content.contains("[languages.go]"));

    assert!(config_content.contains("[languages.vue.grammar]"));
    assert!(config_content.contains("[languages.dockerfile.grammar]"));
    assert!(config_content.contains("[languages.swift.grammar]"));
    assert!(config_content.contains("tree-sitter-vue"));
    assert!(config_content.contains("tree-sitter-dockerfile"));
    assert!(config_content.contains("tree-sitter-swift"));

    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&config_content);
    assert!(
        parsed_config.is_ok(),
        "Generated config should be valid TOML"
    );

    let config = parsed_config.unwrap();
    assert!(
        !config.languages.is_empty(),
        "Should have detected languages"
    );

    println!("✅ Smart init test passed");
}

fn test_comprehensive_init(project_dir: &std::path::Path) {
    println!("Testing comprehensive init...");

    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args([
            "init",
            "--comprehensive",
            "--output",
            "comprehensive-config.toml",
            "--force",
        ])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Comprehensive init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_path = project_dir.join("comprehensive-config.toml");
    assert!(
        config_path.exists(),
        "Comprehensive config file was not created"
    );

    let config_content = fs::read_to_string(&config_path).unwrap();

    assert!(!config_content.contains("# Comprehensive Uncomment Configuration"));

    assert!(config_content.contains("[languages.vue]"));
    assert!(config_content.contains("[languages.svelte]"));
    assert!(config_content.contains("[languages.swift]"));
    assert!(config_content.contains("[languages.kotlin]"));
    assert!(config_content.contains("[languages.dart]"));
    assert!(config_content.contains("[languages.zig]"));
    assert!(config_content.contains("[languages.haskell]"));
    assert!(config_content.contains("[languages.elixir]"));
    assert!(config_content.contains("[languages.julia]"));
    assert!(config_content.contains("[languages.r]"));

    assert!(config_content.contains("source = { type = \"git\""));

    assert!(!config_content.contains("# Web Development Languages"));
    assert!(!config_content.contains("# Mobile Development"));

    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&config_content);
    assert!(
        parsed_config.is_ok(),
        "Comprehensive config should be valid TOML"
    );

    let config = parsed_config.unwrap();
    assert!(
        config.languages.len() >= 10,
        "Should have many languages in comprehensive config, got: {}",
        config.languages.len()
    );

    println!("✅ Comprehensive init test passed");
}

fn test_generated_config_processing(project_dir: &std::path::Path) {
    println!("Testing generated config can process files...");

    let config_path = project_dir.join("smart-config.toml");

    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--dry-run",
            "--verbose",
            "src/main.rs",
            "frontend/app.js",
            "scripts/build.py",
        ])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute processing command");

    println!(
        "Processing output: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    if !output.status.success() {
        println!(
            "Processing stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    assert!(
        output.status.success(),
        "File processing with generated config failed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("files processed") || stdout.contains("comment-free"),
        "Should indicate files were processed: {}",
        stdout
    );

    assert!(
        stdout.contains("files") || stdout.contains("Processing") || stdout.contains("comments"),
        "Should show processing activity"
    );

    println!("✅ Generated config processing test passed");
}

fn test_force_overwrite(project_dir: &std::path::Path) {
    println!("Testing force overwrite...");

    let config_path = project_dir.join("test-force.toml");

    let binary_path = get_binary_path();
    let output1 = Command::new(&binary_path)
        .args(["init", "--output", "test-force.toml"])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute first init command");

    assert!(output1.status.success(), "First init command failed");
    assert!(config_path.exists(), "Config file should be created");

    let original_content = fs::read_to_string(&config_path).unwrap();

    let output2 = Command::new(&binary_path)
        .args(["init", "--output", "test-force.toml"])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute second init command");

    assert!(
        !output2.status.success(),
        "Second init without force should fail"
    );

    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("Use --force"),
        "Should show error about existing file"
    );

    let output3 = Command::new(&binary_path)
        .args([
            "init",
            "--comprehensive",
            "--output",
            "test-force.toml",
            "--force",
        ])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute third init command");

    assert!(
        output3.status.success(),
        "Third init with force should succeed"
    );

    let new_content = fs::read_to_string(&config_path).unwrap();
    assert_ne!(
        original_content, new_content,
        "Content should be different after force overwrite"
    );
    assert!(
        new_content.contains("[languages.vue]") && new_content.contains("[languages.kotlin]"),
        "Should have comprehensive config after force overwrite"
    );

    println!("✅ Force overwrite test passed");
}

/// Test error handling scenarios
#[test]
fn test_init_error_scenarios() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args(["init", "--output", "/nonexistent/path/config.toml"])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "Should fail with invalid output path"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory") || stderr.contains("Failed"),
        "Should show error about invalid path"
    );

    println!("✅ Error scenarios test passed");
}

/// Test help output
#[test]
fn test_init_help() {
    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args(["init", "--help"])
        .output()
        .expect("Failed to execute help command");

    assert!(output.status.success(), "Help command should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Create a template configuration file"));
    assert!(stdout.contains("--comprehensive"));
    assert!(stdout.contains("--interactive"));
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("--output"));

    println!("✅ Help test passed");
}

/// Test that comprehensive config includes expected language repositories
#[test]
fn test_comprehensive_config_repositories() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path();

    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args(["init", "--comprehensive", "--output", "repo-test.toml"])
        .current_dir(project_dir)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Comprehensive init should succeed");

    let config_content = fs::read_to_string(project_dir.join("repo-test.toml")).unwrap();

    assert!(config_content.contains("https://github.com/ikatyang/tree-sitter-vue"));
    assert!(config_content.contains("https://github.com/alex-pinkus/tree-sitter-swift"));
    assert!(config_content.contains("https://github.com/fwcd/tree-sitter-kotlin"));
    assert!(config_content.contains("https://github.com/Himujjal/tree-sitter-svelte"));
    assert!(config_content.contains("https://github.com/tree-sitter/tree-sitter-haskell"));
    assert!(config_content.contains("https://github.com/elixir-lang/tree-sitter-elixir"));

    assert!(config_content.contains("type = \"git\""));

    println!("✅ Repository URLs test passed");
}
