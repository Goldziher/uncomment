// TypeScript Declaration file
// Type definitions
declare module "test-module" {
  // Interface comment
  export interface Person {
    // Property comment
    name: string;
    age: number;
  }

  // Function comment
  export function greet(person: Person): string;
}
