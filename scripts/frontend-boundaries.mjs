import { cwd } from "node:process";
import { dirname, relative, resolve, sep } from "node:path";
import ts from "typescript";

const local = (path) => path.split(sep).join("/");
/** Resolve every import form before comparing layers, including alternate relative paths. */
export function frontendViolations(file, source, root = cwd()) {
  const absolute = resolve(root, file);
  const from = local(relative(root, absolute));
  const application = from.startsWith("src/application/");
  const feature = /^src\/components\/([^/]+)\//u.exec(from)?.[1];
  const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  const violations = [];
  function check(specifier) {
    const resolved = ts.resolveModuleName(
      specifier,
      absolute,
      {
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        resolveJsonModule: true,
      },
      ts.sys,
    ).resolvedModule?.resolvedFileName;
    const target = resolved
      ? local(relative(root, resolved))
      : specifier.startsWith(".")
        ? local(relative(root, resolve(dirname(absolute), specifier)))
        : specifier;
    if (
      application &&
      (/^(react(?:-dom)?(?:\/|$)|@codemirror\/|@tauri-apps\/|node:)/u.test(
        specifier,
      ) ||
        (specifier.startsWith(".") && !target.startsWith("src/application/")) ||
        /^src\/(components|adapters|editor)\//u.test(target))
    )
      violations.push(`application imports outer implementation: ${specifier}`);
    const destination = /^src\/components\/([^/]+)\/(?:.*\/)?use[^/]*$/u.exec(
      target.replace(/\.[cm]?[jt]sx?$/u, ""),
    );
    if (feature && destination && destination[1] !== feature)
      violations.push(`feature imports another feature's hook: ${specifier}`);
  }
  function visit(node) {
    if (
      (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier)
    )
      check(node.moduleSpecifier.text);
    if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument) &&
      ts.isStringLiteral(node.argument.literal)
    )
      check(node.argument.literal.text);
    if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) &&
          node.expression.text === "require")) &&
      node.arguments[0] &&
      ts.isStringLiteral(node.arguments[0])
    )
      check(node.arguments[0].text);
    if (
      application &&
      ts.isTypeReferenceNode(node) &&
      /^(?:HTML\w*Element|Document|Window|KeyboardEvent)$/u.test(
        node.typeName.getText(ast),
      )
    )
      violations.push("application depends on a DOM type");
    if (
      application &&
      ts.isPropertyAccessExpression(node) &&
      /^(?:document|window)$/u.test(node.expression.getText(ast))
    )
      violations.push("application accesses a DOM global");
    ts.forEachChild(node, visit);
  }
  visit(ast);
  return violations;
}
