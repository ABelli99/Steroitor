import { CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { java } from "@codemirror/lang-java";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { javaCompletions } from "./completion";

const CURSOR = "|";

function complete(source: string, explicit = false) {
  const pos = source.indexOf(CURSOR);
  const state = EditorState.create({ doc: source.replace(CURSOR, ""), extensions: [java()] });
  return javaCompletions(new CompletionContext(state, pos, explicit)) as CompletionResult | null;
}

const labels = (source: string, explicit = false) => complete(source, explicit)?.options.map((option) => option.label) ?? [];

const program = (body: string) => `
class Animal {
  protected String name;
  public String speak() { return "..."; }
}

public class Dog extends Animal {
  private int age;
  static int count;
  public static Dog create() { return new Dog(); }
  public void bark(int times) {
${body}
  }
}
`;

describe("javaCompletions", () => {
  it("lists String methods after a String variable", () => {
    const options = labels(program(`    String text = "ciao";\n    text.|`));
    expect(options).toEqual(expect.arrayContaining(["length", "substring", "toUpperCase", "equals"]));
    expect(options).not.toContain("valueOf");
  });

  it("shows parameters next to the method name and the return type as detail", () => {
    const substring = complete(program(`    String text = "ciao";\n    text.|`))?.options.find((option) => option.label === "substring");
    expect(substring?.displayLabel).toBe("substring(int beginIndex, int endIndex)");
    expect(substring?.detail).toBe("String");
    const speak = complete(program(`    this.|`))?.options.find((option) => option.label === "speak");
    expect(speak).toMatchObject({ displayLabel: "speak()", detail: "String" });
  });

  it("lists members of a class declared in the file, including its superclass", () => {
    const options = labels(program(`    Dog other = new Dog();\n    other.|`));
    expect(options).toEqual(expect.arrayContaining(["age", "bark", "speak", "name", "toString"]));
  });

  it("infers the type of var from the constructor and strips generics", () => {
    expect(labels(program(`    var names = new ArrayList<String>();\n    names.|`))).toEqual(expect.arrayContaining(["add", "get", "stream", "size"]));
    expect(labels(program(`    List<String> names = List.of();\n    names.|`))).toContain("subList");
  });

  it("follows chains of fields and calls", () => {
    expect(labels(program(`    System.out.|`))).toContain("println");
    expect(labels(program(`    String text = "a";\n    text.trim().|`))).toContain("isBlank");
    expect(labels(program(`    this.|`))).toEqual(expect.arrayContaining(["age", "bark", "speak"]));
  });

  it("offers only static members on a class name", () => {
    const options = labels(program(`    Dog.|`));
    expect(options).toEqual(expect.arrayContaining(["count", "create"]));
    expect(options).not.toContain("bark");
    expect(labels(program(`    Math.|`))).toContain("sqrt");
  });

  it("knows array length and parameters in scope", () => {
    expect(labels(program(`    int[] values = new int[3];\n    values.|`))).toContain("length");
    expect(labels(program(`    ti|`))).toContain("times");
    expect(labels(program(`    a|`))).toContain("age");
  });

  it("does not show locals outside their scope or completions inside strings and comments", () => {
    expect(labels(`class A {\n  void a() { int hidden = 1; }\n  void b() { hi| }\n}`)).not.toContain("hidden");
    expect(complete(program(`    String s = "te|";`))).toBeNull();
    expect(complete(program(`    // te|`))).toBeNull();
  });

  it("includes keywords and snippets when typing a word", () => {
    expect(labels(program(`    so|`))).toContain("sout");
    expect(labels(program(`    ret|`))).toContain("return");
  });
});
