function testRegex() {
  const simpleRegex = /test/;

  const escapedSlashRegex = /\//;

  const complexRegex = /test\/with\/slashes\/and\swhitespace/;

  const regexWithFlags = /test/gi;

  expect(element).not.toHaveTextContent(/\//);
}
