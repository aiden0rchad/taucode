import "react";

declare module "react" {
  interface TextareaHTMLAttributes<T> {
    /** WebKit inline predictions; a following space accepts them into the text. */
    writingsuggestions?: "true" | "false";
  }
}
