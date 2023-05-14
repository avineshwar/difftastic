#! /usr/bin/env dcli

import 'dart:io';
// import 'package:dcli/dcli.dart';
import 'package:path/path.dart';

var lines = 0;
final sectionSize = 100;
final maxPrint = 500;
var linesPrinted = 0;
var errorLines = 0;
void main(List<String> args) async {
  if (args.length < 2) {
    print("Usage: dart tester/test.dart /path/to/directory/for/testing parse/highlight" );
    return;
  }
  final files = Directory(args[0])
      .listSync(recursive: true)
      .whereType<File>()
      .where((e) =>
          e.path.endsWith('.dart') )
      .map((f) => f.path)
      .toList();

  final results = <Future<void>>[];
  for (var i = 0; i < files.length; i += sectionSize) {
    final sublist = files.sublist(
        i, i + sectionSize < files.length ? i + sectionSize : files.length);
    results.add(runTreeSitter(sublist, args[1]));
  }
  await Future.wait(results);
  print('Processed $lines lines of tree-sitter output');
  print('Error lines $errorLines');
  print('Error percentage ${errorLines * 100 / lines}%');
}

Future<void> runTreeSitterSingle(List<String> files, String parseOrHighlight) async {
  assert(files.length == 1);
  try {
    final result = await Process.run(
        absolute('node_modules/tree-sitter-cli/tree-sitter'),
        [parseOrHighlight, ...files]);
    var lastLine = "";
    var prevLastLine = "";

    var hadError = false;
    var lineErrors = "";
    for (final line in result.stdout.split('\n')) {
      prevLastLine = lastLine;
      lastLine = line + "\n";
      lines++;

      if (line.contains('ERROR')) {
        errorLines++;
        lineErrors += line + "\n";
        hadError = true;
      }
    }
    if (hadError) {
      print(prevLastLine);
      print(lineErrors);
    }
  } catch (e) {
    print('Serious error on $parseOrHighlight in $files');
    print(e);
  }
}

Future<void> runTreeSitter(List<String> files, String parseOrHighlight) async {
  try {
  final result = await Process.run(
      absolute('node_modules/tree-sitter-cli/tree-sitter'),
      [parseOrHighlight, ...files]);

  for (final line in result.stdout.split('\n')) {
    lines++;

    if (line.contains('ERROR')) {
      errorLines++;
      if (linesPrinted < maxPrint) {
        linesPrinted++;
        print(line);
      }
    }
  }
  } catch (e){
    print('Serious error on $parseOrHighlight in $files');
    print(e);
  }
}
