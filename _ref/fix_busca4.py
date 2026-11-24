"""`onNearEnd` no EntryList: avisa quando a rolagem chega perto do fim."""

import io

P = "C:/Users/Lugia/Projects/rayclip/src/components/EntryList.tsx"
s = io.open(P, encoding="utf-8").read()

old = '''  onContextMenu,
  renamingId,
  onRename,
  t,
}: {'''
new = '''  onContextMenu,
  renamingId,
  onRename,
  onNearEnd,
  t,
}: {'''
assert old in s
s = s.replace(old, new, 1)

old = '''  onContextMenu: (id: string) => void;'''
new = '''  onContextMenu: (id: string) => void;
  /** Chamado quando a rolagem chega perto do fim (pede a próxima página). */
  onNearEnd?: () => void;'''
assert old in s
s = s.replace(old, new, 1)

old = '''    const onScroll = () => setScrollTop(el.scrollTop);'''
new = '''    const onScroll = () => {
      setScrollTop(el.scrollTop);
      // Faltando 3 telas de rolagem para o fim: pede mais. O `loadMore` do App
      // ignora o pedido se não houver mais nada ou se já estiver carregando.
      if (
        onNearEnd &&
        el.scrollTop + el.clientHeight >= el.scrollHeight - el.clientHeight * 3
      ) {
        onNearEnd();
      }
    };'''
assert old in s
s = s.replace(old, new, 1)

old = '''  }, [hasEntries]);'''
new = '''  }, [hasEntries, onNearEnd]);'''
assert old in s
s = s.replace(old, new, 1)

io.open(P, "w", encoding="utf-8", newline="").write(s)
print("EntryList: onNearEnd")
