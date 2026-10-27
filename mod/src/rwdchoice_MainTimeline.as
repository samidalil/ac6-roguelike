package _02_GotReward_fla
{
   import flash.display.DisplayObject;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.events.Event;
   import flash.geom.Rectangle;

   // menu/02_rwdchoice.gfx: the reward choice grid. The game's "Pièces obtenues" class drives Dialog/_Content_; the
   // DLL fills _Content_/TileNN (one part each) and writes "selected/count" into _Content_/SelIdx. This script only
   // lays the grid out (3 columns, up to 4 rows), resizes the window and moves the highlight. The movie gets no input.
   [SWF(width="1920", height="1080", backgroundColor="#787878", frameRate="30")]
   public dynamic class MainTimeline extends MovieClip
   {
      public var Dialog:MovieClip;

      private static const COLS:int = 3;
      private static const MAX:int = 12;
      private static const TILE_SCALE:Number = 0.42;
      private static const GAP:Number = 14;
      // one part's box in _Content_ coordinates (thumbnail frame, logo, texts), pixels
      private static const BOX:Rectangle = new Rectangle(30, 40, 540, 230);

      private var orig:Object = null;   // vanilla positions / sizes, read once
      private var shown:String = "";     // last state applied
      private var GROW:Boolean = true;
      private var hl:Shape = null;        // selection frame (drawn: the movie has no fitting symbol)   // window resize (step 2)

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

      private function tick(e:Event) : void
      {
         var c:MovieClip = this.Dialog ? this.Dialog._Content_ as MovieClip : null;
         if (c == null || c.SelIdx == null || this.tile(c, 0) == null) return;
         var state:String = c.SelIdx.text;
         var parts:Array = state.split("/");
         var sel:int = parseInt(parts[0]);
         var n:int = parts.length > 1 ? parseInt(parts[1]) : MAX;
         if (isNaN(sel)) sel = 0;
         if (isNaN(n) || n < 1 || n > MAX) n = MAX;
         if (state == this.shown) return;
         this.shown = state;
         if (this.orig == null) this.remember(c);
         // the single-part slots the game fills stay bound but hidden
         c.ItemName.visible = false; c.CategoryName.visible = false; c.ItemIcon.visible = false;
         c.MakerLogo.visible = false; c.SelIdx.visible = false;
         var tb:Rectangle = BOX;
         var pw:Number = tb.width * TILE_SCALE + GAP;
         var ph:Number = tb.height * TILE_SCALE + GAP;
         var cols:int = Math.min(COLS, n);
         var rows:int = Math.ceil(n / COLS);
         var x0:Number = this.orig.bg != null ? this.orig.bg.x + (this.orig.bg.width - cols * pw + GAP) / 2 : tb.x;
         var y0:Number = tb.y;
         for (var i:int = 0; i < MAX; i++)
         {
            var t:MovieClip = this.tile(c, i);
            t.visible = i < n;
            t.scaleX = t.scaleY = TILE_SCALE;
            t.x = x0 - tb.x * TILE_SCALE + (i % COLS) * pw;
            t.y = y0 - tb.y * TILE_SCALE + int(i / COLS) * ph;
            t.Sel.visible = i == sel;
            t.Sel.visible = false;
         }
         if (this.GROW) this.grow(cols * pw, rows * ph);
         this.highlight(c, this.tile(c, Math.min(sel, n - 1)));
      }

      // frame around the selected tile's maker logo + thumbnail
      private function highlight(c:MovieClip, t:MovieClip) : void
      {
         if (this.hl == null) { this.hl = new Shape(); c.addChild(this.hl); }
         var r:Rectangle = t.ItemIcon.getBounds(c).union(t.MakerLogo.getBounds(c));
         r.inflate(8, 6);
         this.hl.graphics.clear();
         this.hl.graphics.lineStyle(3, 0xFFE9A0, 1);
         this.hl.graphics.beginFill(0xFFFFFF, 0.08);
         this.hl.graphics.drawRect(r.x, r.y, r.width, r.height);
         this.hl.graphics.endFill();
      }

      // the window grows by the grid's extra width / height over the vanilla content box
      private function grow(w:Number, h:Number) : void
      {
         var dw:Number = Math.max(0, w - this.orig.width);
         var dh:Number = h - this.orig.height;
         for each (var nm:String in ["Bg0", "Bg1", "Bg2", "Bg3"])
         {
            var o:Object = this.orig[nm];
            var d:DisplayObject = this.Dialog[nm];
            if (d == null || o == null) continue;
            d.width = o.w + dw;
            d.height = o.h + dh;
         }
         for each (var b:String in ["Line0", "Button", "HitArea"])
         {
            var bo:DisplayObject = this.Dialog[b];
            if (bo != null && this.orig[b] != null) { bo.y = this.orig[b].y + dh; bo.x = this.orig[b].x + dw / 2; }
         }
      }

      private function remember(c:MovieClip) : void
      {
         var o:Object = {};
         var t:MovieClip = this.tile(c, 0);
         var s:Number = t.scaleX;
         t.scaleX = t.scaleY = 1;
         o.tile = t.getBounds(t);
         var box:Rectangle = c.ItemIcon.getBounds(c).union(c.ItemName.getBounds(c)).union(c.MakerLogo.getBounds(c));
         o.left = box.x; o.top = box.y; o.width = box.width; o.height = box.height;
         for each (var nm:String in ["Bg0", "Bg1", "Bg2", "Bg3", "Line0", "Line1", "Button", "HitArea", "Title"])
         {
            var d:DisplayObject = this.Dialog[nm];
            if (d != null) o[nm] = { x: d.x, y: d.y, w: d.width, h: d.height };
         }
         o.dx = this.Dialog.x; o.dy = this.Dialog.y;
         o.bg = this.Dialog.Bg0 != null ? this.Dialog.Bg0.getBounds(c) : null;
         this.orig = o;
      }
   }
}
