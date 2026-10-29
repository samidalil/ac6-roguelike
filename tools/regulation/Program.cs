// RegBuild <game regulation.bin> <data dir> <out regulation.bin> [<Smithbox AC6 defs dir>]
// Applies the declarations of every *.toml in the data dir to the game's regulation and writes an encrypted copy
// (served by me3 from the mod package). Declarations:
//   [ParamName.rowId]        a row of a param (file name in the regulation, e.g. EquipParamWeapon)
//   copy = <rowId>           new row only: start from a copy of this row (else all fields 0)
//   field = value            Smithbox field name (case ignored) = number or "text"
// A row that does not exist is added (rows are kept sorted by id). Errors stop the build (nothing written).
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using SoulsFormats;

class RegBuild
{
    record Decl(string File, int Line, string Param, int Row, Dictionary<string, (string Value, int Line)> Fields);

    static int Main(string[] a)
    {
        if (a.Length < 3) { Console.Error.WriteLine("usage: RegBuild <game regulation.bin> <data dir> <out regulation.bin> [<AC6 defs dir>]"); return 2; }
        string src = a[0], dataDir = a[1], dst = a[2];
        string defs = a.Length > 3 ? a[3] : Path.Combine(Path.GetDirectoryName(typeof(PARAM).Assembly.Location)!, "Assets", "PARAM", "AC6", "Defs");
        var decls = new List<Decl>();
        foreach (var f in Directory.GetFiles(dataDir, "*.toml").OrderBy(x => x)) decls.AddRange(Parse(f));
        if (decls.Count == 0) { Console.WriteLine("no declarations: nothing to build"); return 0; }

        var bnd = SFUtil.DecryptAC6Regulation(src);
        var errors = new List<string>();
        int edited = 0, added = 0, fields = 0;
        foreach (var g in decls.GroupBy(d => d.Param, StringComparer.OrdinalIgnoreCase))
        {
            var file = bnd.Files.FirstOrDefault(x => string.Equals(Path.GetFileNameWithoutExtension(x.Name), g.Key, StringComparison.OrdinalIgnoreCase));
            if (file == null) { errors.Add($"{g.First().File}:{g.First().Line}: no param {g.Key} in the regulation"); continue; }
            var name = Path.GetFileNameWithoutExtension(file.Name);
            var p = PARAM.Read(file.Bytes);
            var defPath = new[] { name, name + "Param", name.EndsWith("Param") ? name[..^5] : name }.Select(x => Path.Combine(defs, x + ".xml"))
                .FirstOrDefault(File.Exists) ?? (p.ParamType != null && DefPaths(defs).TryGetValue(p.ParamType, out var byType) ? byType : null);
            if (defPath == null) { errors.Add($"no Smithbox definition for {name} in {defs}"); continue; }
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(defPath));
            foreach (var d in g)
            {
                var row = p.Rows.FirstOrDefault(r => r.ID == d.Row);
                if (row == null)
                {
                    row = new PARAM.Row(d.Row, "", p.AppliedParamdef);
                    if (d.Fields.TryGetValue("copy", out var c))
                    {
                        if (!int.TryParse(c.Value, out var from) || p.Rows.FirstOrDefault(r => r.ID == from) is not PARAM.Row orig)
                        { errors.Add($"{d.File}:{c.Line}: copy = {c.Value}: no row {c.Value} in {name}"); continue; }
                        foreach (var (cell, i) in orig.Cells.Select((x, i) => (x, i))) row.Cells[i].Value = cell.Value;
                    }
                    p.Rows.Add(row);
                    added++;
                }
                else edited++;
                foreach (var (k, (v, line)) in d.Fields)
                {
                    if (k.Equals("copy", StringComparison.OrdinalIgnoreCase)) continue;
                    var cell = row.Cells.FirstOrDefault(x => x.Def.InternalName.Equals(k, StringComparison.OrdinalIgnoreCase));
                    if (cell == null) { errors.Add($"{d.File}:{line}: {name} has no field {k}"); continue; }
                    try { cell.Value = Convert(cell.Def.DisplayType, v); fields++; }
                    catch (Exception e) { errors.Add($"{d.File}:{line}: {k} = {v}: {e.Message}"); }
                }
            }
            p.Rows.Sort((x, y) => x.ID.CompareTo(y.ID));
            file.Bytes = p.Write();
        }
        if (errors.Count > 0) { foreach (var e in errors) Console.Error.WriteLine("error: " + e); return 1; }
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(dst))!);
        SFUtil.EncryptAC6Regulation(dst, bnd, bnd.Compression);
        Console.WriteLine($"{dst}: {decls.Count} declaration(s), {edited} row(s) edited, {added} added, {fields} field(s) set");
        return 0;
    }

    /// Smithbox definition files by their ParamType, for a param whose file name matches no variant of its name.
    static Dictionary<string, string> defPaths;
    static Dictionary<string, string> DefPaths(string dir)
    {
        if (defPaths != null) return defPaths;
        defPaths = new();
        var rx = new System.Text.RegularExpressions.Regex("<ParamType>([^<]+)</ParamType>");
        foreach (var f in Directory.GetFiles(dir, "*.xml"))
        {
            var m = rx.Match(File.ReadAllText(f));
            if (m.Success) defPaths.TryAdd(m.Groups[1].Value.Trim(), f);
        }
        return defPaths;
    }

    static object Convert(PARAMDEF.DefType t, string v) => t switch
    {
        PARAMDEF.DefType.s8 => sbyte.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.u8 => byte.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.s16 => short.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.u16 => ushort.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.s32 or PARAMDEF.DefType.b32 => int.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.u32 => uint.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.f32 or PARAMDEF.DefType.angle32 => float.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.f64 => double.Parse(v, CultureInfo.InvariantCulture),
        PARAMDEF.DefType.fixstr or PARAMDEF.DefType.fixstrW => v,
        _ => throw new FormatException($"field type {t} not supported"),
    };

    /// A tiny TOML subset: [Param.row] sections, key = value (number or "text"), # comments outside quotes.
    static IEnumerable<Decl> Parse(string path)
    {
        Decl cur = null;
        var lines = File.ReadAllLines(path);
        for (int n = 0; n < lines.Length; n++)
        {
            var line = StripComment(lines[n]).Trim();
            if (line.Length == 0) continue;
            if (line.StartsWith('[') && line.EndsWith(']'))
            {
                if (cur != null) yield return cur;
                var s = line[1..^1].Trim().Trim('"');
                var dot = s.LastIndexOf('.');
                if (dot <= 0 || !int.TryParse(s[(dot + 1)..], out var row)) throw new FormatException($"{path}:{n + 1}: [ParamName.rowId] expected, not [{s}]");
                cur = new Decl(Path.GetFileName(path), n + 1, s[..dot], row, new(StringComparer.OrdinalIgnoreCase));
                continue;
            }
            var eq = line.IndexOf('=');
            if (eq < 0 || cur == null) throw new FormatException($"{path}:{n + 1}: key = value inside a [ParamName.rowId] section expected");
            var k = line[..eq].Trim().Trim('"');
            var v = line[(eq + 1)..].Trim();
            if (v.Length >= 2 && v[0] == '"' && v[^1] == '"') v = v[1..^1];
            cur.Fields[k] = (v, n + 1);
        }
        if (cur != null) yield return cur;
    }

    static string StripComment(string s)
    {
        bool q = false;
        for (int i = 0; i < s.Length; i++) { if (s[i] == '"') q = !q; else if (s[i] == '#' && !q) return s[..i]; }
        return s;
    }
}
