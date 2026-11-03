<template>
  <div>
    <div class="panel" style="max-width: 560px">
      <div class="panel-title">aria2c 程序 ✿</div>

      <el-form label-position="top" size="large">
        <el-form-item label="aria2c.exe 路径">
          <div class="row">
            <el-input v-model="s.aria2_path" placeholder="留空 = 使用 PATH 中的 aria2c" clearable />
            <el-button :icon="FolderOpened" @click="browse">浏览…</el-button>
          </div>
          <div class="hint">
            <el-tag :type="found ? 'success' : 'danger'" size="small" effect="light">
              {{ found ? "可用" : "未找到" }}
            </el-tag>
            <span class="hint-text">{{ aria2Hint }}</span>
          </div>
        </el-form-item>
      </el-form>
    </div>

    <div class="panel" style="max-width: 560px">
      <div class="panel-title">下载参数 ✿</div>

      <el-form label-position="top" size="large">
        <el-form-item label="代理地址（所有协议）">
          <el-input v-model="s.proxy" placeholder="http://127.0.0.1:10808，留空 = 不用代理" clearable />
        </el-form-item>

        <div class="grid3">
          <el-form-item label="每服务器连接数 (-x)">
            <el-input-number v-model="s.connections" :min="1" :max="16" style="width: 100%" />
          </el-form-item>
          <el-form-item label="任务分片数 (-s)">
            <el-input-number v-model="s.split" :min="1" :max="1024" style="width: 100%" />
          </el-form-item>
          <el-form-item label="分片大小 (-k)">
            <el-select v-model="s.piece_length" style="width: 100%">
              <el-option v-for="o in ['1M', '2M', '4M', '8M', '16M', '512K', '256K']" :key="o" :label="o" :value="o" />
            </el-select>
          </el-form-item>
        </div>

        <el-form-item label="下载目录 (-d)">
          <div class="row">
            <el-input v-model="s.dir" placeholder="默认：系统下载文件夹" clearable />
            <el-button :icon="FolderOpened" @click="browseDir">浏览…</el-button>
          </div>
        </el-form-item>

        <el-form-item label="额外参数">
          <el-input v-model="s.extra_args" placeholder="如 --check-certificate=false" clearable />
          <div class="hint hint-plain">原样追加到 aria2c 命令末尾，支持引号包裹的参数</div>
        </el-form-item>
      </el-form>
    </div>

    <!-- 界面背景 -->
    <div class="panel" style="max-width: 560px">
      <div class="panel-title">界面背景 ✿</div>
      <el-form label-position="top" size="large">
        <el-form-item label="背景图片">
          <div class="row">
            <el-input v-model="s.bg_image" placeholder="留空 = 默认渐变背景" clearable />
            <el-button :icon="FolderOpened" @click="browseBg">浏览…</el-button>
            <el-button :icon="Delete" :disabled="!s.bg_image" @click="s.bg_image = ''">清除</el-button>
          </div>
        </el-form-item>
        <el-form-item label="图片不透明度">
          <el-slider
            v-model="s.bg_opacity"
            :min="0" :max="1" :step="0.05"
            :disabled="!s.bg_image"
            :format-tooltip="fmtPct"
          />
        </el-form-item>
        <el-form-item label="页面整体不透明度（越低背景越透）">
          <el-slider v-model="s.ui_opacity" :min="0.2" :max="1" :step="0.05" :format-tooltip="fmtPct" />
        </el-form-item>
      </el-form>
      <div class="hint hint-plain">保存后立即生效</div>
    </div>

    <!-- 保存：作用于以上所有设置 -->
    <div class="row" style="max-width: 560px; margin-top: 4px">
      <el-button type="primary" round class="pink-btn" :icon="Check" @click="save">保存设置</el-button>
      <el-button round :icon="RefreshLeft" @click="resetDefaults">恢复默认</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { FolderOpened, Check, RefreshLeft, Delete } from "@element-plus/icons-vue";

interface Settings {
  aria2_path: string;
  proxy: string;
  connections: number;
  split: number;
  piece_length: string;
  dir: string;
  extra_args: string;
  bg_image: string;
  bg_opacity: number;
  ui_opacity: number;
}

const s = reactive<Settings>({
  aria2_path: "",
  proxy: "http://127.0.0.1:10808",
  connections: 16,
  split: 16,
  piece_length: "1M",
  dir: "",
  extra_args: "",
  bg_image: "",
  bg_opacity: 0.5,
  ui_opacity: 1,
});

const fmtPct = (v: number) => Math.round(v * 100) + "%";

const found = ref(false);
const aria2Hint = ref("");

onMounted(async () => {
  try {
    const saved: Settings = await invoke("load_settings");
    Object.assign(s, saved);
  } catch (e) {
    console.warn("load_settings failed", e);
  }
  recheck();
});

async function save() {
  try {
    await invoke("save_settings", { settings: s });
    ElMessage.success("设置已保存");
    recheck();
  } catch (e) {
    ElMessage.error(`保存失败：${e}`);
  }
}

async function resetDefaults() {
  const d: Settings = await invoke("default_settings");
  Object.assign(s, d);
  ElMessage.info("已恢复默认值（记得点保存生效）");
}

async function recheck() {
  try {
    const p: string = await invoke("find_aria2");
    found.value = true;
    aria2Hint.value = p;
  } catch {
    found.value = false;
    aria2Hint.value = "未在 PATH 中找到，也未在上方指定路径";
  }
}

async function browse() {
  const picked = await open({
    multiple: false,
    filters: [{ name: "aria2c", extensions: ["exe"] }],
  });
  if (typeof picked === "string") {
    s.aria2_path = picked;
    recheck();
  }
}

async function browseDir() {
  const picked = await open({ directory: true, multiple: false });
  if (typeof picked === "string") s.dir = picked;
}

async function browseBg() {
  const picked = await open({
    multiple: false,
    filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
  });
  if (typeof picked === "string") s.bg_image = picked;
}
</script>

<style scoped>
.row { display: flex; gap: 8px; width: 100%; }
.grid3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
.hint { margin-top: 4px; display: flex; align-items: center; gap: 8px; }
.hint-plain { margin-top: 4px; font-size: 12px; color: rgba(255, 255, 255, .8); }
.hint-text { font-size: 12px; color: rgba(255, 255, 255, .8); word-break: break-all; }

/* 主按钮：淡粉玻璃，与幽灵风统一 */
.pink-btn {
  background: rgba(232, 146, 208, .28) !important;
  border: 1px solid rgba(255, 255, 255, .3) !important;
  color: #fff !important;
  transition: all .2s ease !important;
}
.pink-btn:hover:not(:disabled) {
  background: rgba(232, 146, 208, .42) !important;
  transform: translateY(-1px);
}
.pink-btn:active:not(:disabled) { transform: translateY(0) scale(.98); }
</style>
