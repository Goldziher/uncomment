/**
 * Type declarations for Calculator library
 */

declare module "calculator" {
  /**
   * Adds two numbers
   * @param a - First number
   * @param b - Second number
   * @returns Sum of a and b
   */
  export function add(a: number, b: number): number;

  /**
   * Multiplies two numbers
   * @param a - First number
   * @param b - Second number
   * @returns Product of a and b
   */
  export function multiply(a: number, b: number): number;

  export class Calculator {
    result: number;

    constructor();

    clear(): void;
  }
}
