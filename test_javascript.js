const add = (a, b) => {
  return a + b;
};

/**
 * Multiply two numbers
 * @param {number} a - First number
 * @param {number} b - Second number
 * @returns {number} - Result of multiplication
 */
function multiply(a, b) {
  return a * b;
}

// TODO: Add more functions
// FIXME: Fix the bug in divide function

class Calculator {
  constructor() {
    this.result = 0;
  }

  clear() {
    this.result = 0;
  }
}

export { add, multiply, Calculator };
