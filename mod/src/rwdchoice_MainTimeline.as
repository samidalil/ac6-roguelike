package _02_GotReward_fla
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.geom.Rectangle;
   import flash.text.TextField;

   // menu/02_rwdchoice.gfx: the reward choice grid. The game's "Pièces obtenues" class drives Dialog/_Content_; the
   // DLL fills _Content_/TileNN (one part each) and writes "selected/count" into _Content_/SelIdx. This script only
   // lays the grid out (3 columns, up to 4 rows), resizes the window around it and draws the highlight. The movie gets
   // no input.
   [SWF(width="1920", height="1080", backgroundColor="#787878", frameRate="30")]
   public dynamic class MainTimeline extends MovieClip
   {
      public var Dialog:MovieClip;

      private static const COLS:int = 3;
      private static const MAX:int = 12;
      // tiles at most this fraction of the vanilla part cell, smaller when the grid would not fit the screen
      private static const TILE_SCALE:Number = 0.9;
      private static const GRID_MAX_W:Number = 1560;
      // window side margin around the grid (the vanilla one, ~210, is too wide for 3 cells)
      private static const PAD:Number = 110;
      private static const GRID_MAX_H:Number = 660;
      private static const GAP:Number = 28;

      private var orig:Object = null;   // vanilla positions / sizes, read once
      private var shown:String = "";     // last state applied
      private var hl:Shape = null;        // selection frame (drawn: the movie has no fitting symbol)
      private var backing:Shape = null;   // dark backing under the window
      private var content:MovieClip = null; // content clip laid out last
      private var gridW:Number = 0;       // size of the grid laid out last
      private var gridH:Number = 0;

      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1,22,this.frame23,26,this.frame27);
      }

      internal function frame1() : *
      {
         stop();
         this.addEventListener(Event.ENTER_FRAME, this.tick);
      }

      internal function frame23() : *
      {
         stop();
      }

      internal function frame27() : *
      {
         stop();
      }

      private function tile(c:MovieClip, i:int) : MovieClip
      {
         return c["Tile" + (i < 10 ? "0" : "") + i] as MovieClip;
      }

      // a tile's part slots, in the tile's own coordinates
      // a tile's extent: the logo and the image box (background, frame); not the text fields (much wider than their
      // text) nor the masked image (its bounds are the whole texture)
      private function slotBounds(t:MovieClip) : Rectangle
      {
         var r:Rectangle = null;
         for (var k:int = 0; k < t.numChildren; k++)
         {
            var d:DisplayObject = t.getChildAt(k);
            if (d is TextField || d == t.ItemIcon) continue;
            var b:Rectangle = d.getBounds(t);
            r = r == null ? b : r.union(b);
         }
         return r;
      }

      private function tick(e:Event) : void
      {
         var c:MovieClip = this.Dialog ? this.Dialog._Content_ as MovieClip : null;
         if (c == null || c.SelIdx == null || this.tile(c, 0) == null) return;
         // the single-part slots the game fills stay bound but hidden (every frame: the game shows a page's slots again
         // when it fills them)
         c.ItemName.visible = false; c.CategoryName.visible = false; c.ItemIcon.visible = false;
         c.MakerLogo.visible = false; c.SelIdx.visible = false;
         var state:String = c.SelIdx.text;
         var parts:Array = state.split("/");
         var sel:int = parseInt(parts[0]);
         var n:int = parts.length > 1 ? parseInt(parts[1]) : MAX;
         if (isNaN(sel)) sel = 0;
         if (isNaN(n) || n < 1 || n > MAX) n = MAX;
         // each page is a new content clip in the same window: lay it out even when its state text is the same
         if (c != this.content) { this.content = c; this.shown = ""; this.hl = null; if (this.orig != null) this.rememberContent(c, this.orig); }
         // the window's own animation keyframes reset some of its pieces: size them again every frame
         if (state == this.shown) { if (this.gridW > 0) this.grow(this.gridW, this.gridH); return; }
         this.shown = state;
         if (this.orig == null) this.remember(c);
         var tb:Rectangle = this.orig.tb;
         var cols:int = Math.min(COLS, n);
         var rows:int = Math.ceil(n / COLS);
         var sc:Number = Math.min(TILE_SCALE, (GRID_MAX_W - (cols - 1) * GAP) / (cols * tb.width), (GRID_MAX_H - (rows - 1) * GAP) / (rows * tb.height));
         var pw:Number = tb.width * sc + GAP;
         var ph:Number = tb.height * sc + GAP;
         var gw:Number = cols * pw - GAP;
         var gh:Number = rows * ph - GAP;
         // the grid takes the vanilla content box's place, centred on the window
         var x0:Number = this.orig.mid - gw / 2;
         var y0:Number = this.orig.top;
         for (var i:int = 0; i < MAX; i++)
         {
            var t:MovieClip = this.tile(c, i);
            t.visible = i < n;
            t.scaleX = t.scaleY = sc;
            t.x = x0 - tb.x * sc + (i % COLS) * pw;
            t.y = y0 - tb.y * sc + int(i / COLS) * ph;
         }
         this.gridW = gw; this.gridH = gh;
         this.grow(gw, gh);
         this.highlight(c, this.tile(c, Math.min(sel, n - 1)));
      }

      // frame around the selected tile's slots
      private function highlight(c:MovieClip, t:MovieClip) : void
      {
         if (this.hl == null) { this.hl = new Shape(); c.addChild(this.hl); }
         var b:Rectangle = this.orig.tb;
         var r:Rectangle = new Rectangle(t.x + b.x * t.scaleX, t.y + b.y * t.scaleY, b.width * t.scaleX, b.height * t.scaleY);
         r.inflate(8, 6);
         this.hl.graphics.clear();
         this.hl.graphics.lineStyle(3, 0xE8EEF2, 0.95);
         this.hl.graphics.beginFill(0xFFFFFF, 0.08);
         this.hl.graphics.drawRect(r.x, r.y, r.width, r.height);
         this.hl.graphics.endFill();
      }

      // the window grows (or shrinks in height) by the grid's size over the vanilla content box, around its centre
      private function grow(w:Number, h:Number) : void
      {
         var dw:Number = Math.max(0, w + 2 * PAD - this.orig.bgw);
         var dh:Number = h - this.orig.height;
         var nm:String;
         var o:Object;
         var d:DisplayObject;
         for each (o in this.orig.bgs)
         {
            d = o.d;
            d.width = o.w + dw; d.height = o.h + dh;
            // place by bounds (the pieces' origins are not their corners): dw / 2 more on each side
            var b:Rectangle = d.getBounds(this.Dialog);
            d.x += o.b.x - dw / 2 - b.x; d.y += o.b.y - dh / 2 - b.y;
         }
         // the window is see-through and now wider than the screens it opens over (the results panel showed through
         // as a darker middle): a dark backing under it
         var bb:Rectangle = this.Dialog.Bg0 != null ? this.Dialog.Bg0.getBounds(this.Dialog) : null;
         if (bb != null)
         {
            if (this.backing == null) { this.backing = new Shape(); this.Dialog.addChildAt(this.backing, 0); }
            this.backing.graphics.clear();
            this.backing.graphics.beginFill(0x0A0F16, 0.85);
            this.backing.graphics.drawRect(bb.x, bb.y, bb.width, bb.height);
            this.backing.graphics.endFill();
         }
         // top part (title, top line, content) moves up, bottom part (line, OK) down
         for each (nm in ["Title", "Line1", "_Content_"])
         {
            o = this.orig[nm]; d = this.Dialog[nm];
            if (d != null && o != null) d.y = o.y - dh / 2;
         }
         for each (nm in ["Line0", "Button", "HitArea"])
         {
            o = this.orig[nm]; d = this.Dialog[nm];
            if (d != null && o != null) d.y = o.y + dh / 2;
         }
         for each (nm in ["Line0", "Line1"])
         {
            o = this.orig[nm]; d = this.Dialog[nm];
            if (d != null && o != null) { d.width = o.w + dw; d.x = o.x - dw / 2; }
         }
      }

      // a page's content: one tile's slots (its size) and the vanilla content box (one part's slots in _Content_)
      private function rememberContent(c:MovieClip, o:Object) : Rectangle
      {
         var t:MovieClip = this.tile(c, 0);
         t.scaleX = t.scaleY = 1;
         o.tb = this.slotBounds(t);
         var box:Rectangle = c.ItemIcon.getBounds(c).union(c.ItemName.getBounds(c)).union(c.MakerLogo.getBounds(c));
         o.top = box.y; o.width = box.width; o.height = box.height;
         return box;
      }

      private function remember(c:MovieClip) : void
      {
         var o:Object = {};
         var box:Rectangle = this.rememberContent(c, o);
         for each (var nm:String in ["Line0", "Line1", "Button", "HitArea", "Title", "_Content_"])
         {
            var d:DisplayObject = this.Dialog[nm];
            if (d != null) o[nm] = { x: d.x, y: d.y, w: d.width, h: d.height, b: d.getBounds(this.Dialog) };
         }
         // the window's background pieces: the named ones and its plain shapes (a shape cannot carry an instance name)
         o.bgs = [];
         for (var k:int = 0; k < this.Dialog.numChildren; k++)
         {
            var ch:DisplayObject = this.Dialog.getChildAt(k);
            var named:Boolean = ch.name.length == 3 && ch.name.substr(0, 2) == "Bg";
            if (ch == this.backing || !(named || ch is Shape)) continue;
            o.bgs.push({ d: ch, w: ch.width, h: ch.height, b: ch.getBounds(this.Dialog) });
         }
         var bg:Rectangle = this.Dialog.Bg0 != null ? this.Dialog.Bg0.getBounds(c) : box;
         o.mid = bg.x + bg.width / 2;
         o.bgw = bg.width;
         this.orig = o;
      }
   }
}
