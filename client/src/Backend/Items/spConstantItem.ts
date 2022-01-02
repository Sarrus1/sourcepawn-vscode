import {
  CompletionItemKind,
  Range,
  CompletionItem,
  SignatureInformation,
  Hover,
  DocumentSymbol,
  Location,
  LocationLink,
} from "vscode";

import { SPItem } from "./spItems";

export class ConstantItem implements SPItem {
  name: string;
  kind = CompletionItemKind.Constant;
  calls: Location[];

  constructor(name: string) {
    this.name = name;
    this.calls = [];
  }

  toCompletionItem(): CompletionItem {
    return {
      label: this.name,
      kind: this.kind,
      detail: "",
    };
  }

  toDefinitionItem(): LocationLink {
    return;
  }

  toSignature(): SignatureInformation {
    return;
  }

  toHover(): Hover {
    return;
  }

  toDocumentSymbol(): DocumentSymbol {
    return;
  }
}