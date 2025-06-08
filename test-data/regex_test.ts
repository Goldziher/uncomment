// Test file for regex patterns
function testRegex() {
  // Simple regex
  const simpleRegex = /test/;
  
  // Regex with escaped forward slash
  const escapedSlashRegex = /\//;
  
  // Regex with multiple escape sequences
  const complexRegex = /test\/with\/slashes\/and\swhitespace/;
  
  // Regex with flags
  const regexWithFlags = /test/gi;
  
  // In a test context
  expect(element).not.toHaveTextContent(/\//);
}