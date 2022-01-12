﻿import { basename } from "path";

import { Parser } from "./spParser";
import { EnumStructItem } from "../Backend/Items/spEnumStructItem";
import { EnumItem } from "../Backend/Items/spEnumItem";
import { EnumMemberItem } from "../Backend/Items/spEnumMemberItem";
import { State } from "./stateEnum";
import { searchForDefinesInString } from "./searchForDefinesInString";
import { parseDocComment } from "./parseDocComment";
import { addFullRange } from "./addFullRange";

export function readEnum(
  parser: Parser,
  match: RegExpMatchArray,
  line: string,
  IsStruct: boolean
) {
  let { description, params } = parseDocComment(parser);
  if (IsStruct) {
    // Create a completion for the enum struct itself if it has a name
    let enumStructName = match[1];
    let range = parser.makeDefinitionRange(enumStructName, line);
    var enumStructCompletion: EnumStructItem = new EnumStructItem(
      enumStructName,
      parser.file,
      description,
      range
    );
    parser.completions.set(enumStructName, enumStructCompletion);
    parser.state.push(State.EnumStruct);
    parser.state_data = {
      name: enumStructName,
    };
    return;
  }

  if (!match[1]) {
    parser.anonymousEnumCount++;
  }
  let nameMatch = match[1] ? match[1] : `Enum #${parser.anonymousEnumCount}`;
  let range = parser.makeDefinitionRange(match[1] ? match[1] : "enum", line);
  var enumCompletion: EnumItem = new EnumItem(
    nameMatch,
    parser.file,
    description,
    range
  );
  let key = match[1]
    ? match[1]
    : `${parser.anonymousEnumCount}${basename(parser.file)}`;
  parser.completions.set(key, enumCompletion);

  // Set max number of iterations for safety
  let iter = 0;
  // Match all the enum members
  while (iter < 100 && !/^\s*\}/.test(line)) {
    iter++;
    line = parser.lines.shift();
    parser.lineNb++;
    // Stop early if it's the end of the file
    if (line === undefined) {
      return;
    }
    let iterMatch = line.match(/^\s*(\w*)\s*.*/);

    // Skip if didn't match
    if (!iterMatch) {
      continue;
    }
    let enumMemberName = iterMatch[1];
    // Try to match multiblock comments
    let enumMemberDescription: string;
    iterMatch = line.match(/\/\*\*<?\s*(.+?(?=\*\/))/);
    if (iterMatch) {
      enumMemberDescription = iterMatch[1];
    }
    iterMatch = line.match(/\/\/<?\s*(.*)/);
    if (iterMatch) {
      enumMemberDescription = iterMatch[1];
    }
    let range = parser.makeDefinitionRange(enumMemberName, line);
    parser.completions.set(
      enumMemberName,
      new EnumMemberItem(
        enumMemberName,
        parser.file,
        enumMemberDescription,
        enumCompletion,
        range,
        parser.IsBuiltIn
      )
    );
    searchForDefinesInString(parser, line);
  }
  addFullRange(parser, key);
  return;
}