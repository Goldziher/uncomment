declare module "test-module" {
  export interface Person {
    name: string;
    age: number;
  }

  export function greet(person: Person): string;
}
