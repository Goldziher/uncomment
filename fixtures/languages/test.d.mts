declare module "test-esm-module" {
  export interface User {
    id: number;
    name: string;
  }

  export function findUser(id: number): User;
}
