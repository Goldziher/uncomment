// ESM TypeScript Declaration file
// ESM type definitions
declare module "test-esm-module" {
  // Interface comment
  export interface User {
    // Property comment
    id: number;
    name: string;
  }

  // Function comment
  export function findUser(id: number): User;
}
