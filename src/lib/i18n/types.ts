export type TranslationTree = {
  readonly [key: string]: string | TranslationTree
}

export type TranslationShape<T> = {
  readonly [Key in keyof T]: T[Key] extends string
    ? string
    : T[Key] extends Record<string, unknown>
      ? TranslationShape<T[Key]>
      : never
}

type PreviousDepth = [never, 0, 1, 2, 3, 4, 5]

export type DotNestedLeafKeys<T, Depth extends number = 5> = [
  Depth,
] extends [never]
  ? never
  : {
      [Key in Extract<keyof T, string>]: T[Key] extends string
        ? Key
        : T[Key] extends Record<string, unknown>
          ? `${Key}.${DotNestedLeafKeys<T[Key], PreviousDepth[Depth]>}`
          : never
    }[Extract<keyof T, string>]
