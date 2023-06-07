# TODO list
- 设计
  - [x] 地图动态划分设计
  - [ ] game proto 设计  `Doing`
  - [ ] map proto 设计  `Doing`
  - [ ] 并发安全设计  `Doing`
- dispatcher: 
  - game API impl
    - [x] login
    - [ ] logout
    - [x] aoe
    - [ ] moving  `Doing`
  - 内部机能
    - [ ] overload monitor，监视各个地图服务器负载，发起扩缩容
    - [x] 主导扩容
    - [ ] 主导缩容
- map-server
  - game API impl
    - [ ] login
    - [ ] logout
    - [ ] aoe：为查找区域内用户，要考虑将地图划分为小格，缓存小格内用户
    - [ ] moving
  - map API impl
    - [ ] 扩容：导出一半用户到指定server
    - [ ] 缩容：导出全部用户到指定server
- 其它
  - [ ] config
  - [ ] test
  - [ ] example
  - [ ] benchmark
  - [ ] CI（包括发布docker image）
  - [ ] 扩容时在程序内启动image
  - [ ] 将边缘区域用户同步到其它服务器，提高用户在服务器间移动的性能
  - [ ] 研究一下空间加速算法K-D tree，BVH，Grid等  `Doing`
    - [ ] K-D tree叶子容量数设置调优
  - [ ] 写一个epoch封装的无锁K-D tree

# 架构
服务分两层：
* dispatcher：分发服务器。管理所有地图服务器，后台线程监视各服务器负载，发起扩缩容请求。它有两个缓存：
  * 区域-服务器缓存，用于动态划分区域，负载均衡
  * 用户-服务器缓存，以此为依据将请求分发到地图服务器
* map-server：地图服务器，实现2个service
  * game_service：玩家请求
  * map_service：玩家导入导出等扩缩容相关请求

# 并发一致性
### dispatcher
考虑到写请求不少，不太符合RwLock的应用场景。
为实现无锁，使用跳表（Skiplist）来记录服务器清单、玩家清单。  
另外跳表也无法保证先读后写是原子的，除非上锁，否则存在数据竞争。例如同时两次请求给玩家money+1，可能最后只+1。  
  
解决方案：考虑以玩家为单位将请求串行起来（ert crate）  

对服务器来说，在monitor线程中串行，一次只做一个扩缩容操作
### map-server
打算用kd-tree作为空间加速结构，但是现有的kd-tree crate并不能无锁并发，如果有时间可以用crossbeam-epoch封装一个，
但是现在可能要用RwLock了，这样一来服务器人数就不能太多了，频繁陷入内核调用性能堪忧啊！！！。

# API流程
### login
* dispatcher根据坐标计算分到哪个地图服务器
* 地图服务器login
* dispatcher加入用户缓存

### aoe
* dispatcher根据范围坐标计算分到一台或几台地图服务器
* 各地图服务器处理aoe

### moving
* dispatcher从缓存中取出用户当前zone
* dispatcher计算移动目标zone，
  * 如果不是当前zone：
    * 调用当前地图服务器export_player到目标服务器，`为减轻dispatcher负担，这里不用它中转`
    * dispatcher更新用户-服务器缓存
    * 将walking请求发送给目标服务器
  * 如果是当前zone，则将walking发给原地图服务器

# 数据结构
地图区域划分按照四叉树结构，四个象限1234，递归向下划分 
每次划分都会有四个象限，意味着每个父节点都有满4个子节点。  
叶子结点归地图服务器管理，而且一台服务器所管理的叶子结点必须是相同父节点
同一个叶子节点只有在导入导出时会有2台服务器
### 给定坐标对应区域查找
从根节点一层层算出所在象限向下，直至节点不在在缓存中，返回其父节点

# 动态扩缩容流程
设服务器最大人数MAX（扩容），最低人数MIN（缩容），  

### 扩容
dispatcher监视到某一服务器玩家大于MAX，首先dispatcher启动一台服务器，
* 1. 调用get_heaviest_zone_players，选出最大人数的zone以及其内的用户ID
* 2. 更新区域-服务器缓存，此后该区域请求将转至新服务器
* 3. dispatcher调用export_player，根据用户ID逐个导入新服务器
* 4. dispatcher更新用户-服务器缓存

`3，4须与用户操作API串行，避免数据竞争。`  
这也是3.**逐个**用户导入的原因，一旦进行块传输那么就要给涉及到的所有用户上锁

### 缩容
dispatcher监视到某一服务器玩家小于MIN，尝试缩容。  
缩容是扩容的逆序，同样只能由同父的叶子结点之间合并。当4个叶子结点都归同一台服务器时，其父节点收缩为叶子结点。
  
缺点：缩容的时候只能同父叶子节点合并，如果合并不了，那么负载小的那个也无法和其它父节点下的合并，浪费性能

# 遇到的问题
1. 当扩缩容进行时，玩家导入导出需要时间，此时1个叶子节点可能存在2个服务器(已解决，添加exporting_server记录)
2. 正在扩缩容的服务器dispatcher如何发送game请求，
   * login只发到导入server，moving/logout可根据ert串行等待导出结束。aoe/query无法确定受影响用户，也就无法与导入导出串行，是不是在扩缩容时要给服务器加锁呢？
   * 或者是不是可以aoe/query两个服务器都发
3. 如果玩家都在一个点，无论怎么分割也没用，还是要设置一个最大深度

https://www.cnblogs.com/KillerAery/p/10878367.html#%E7%BD%91%E6%A0%BC-grid
https://zhuanlan.zhihu.com/p/349594815?utm_medium=social&utm_oi=597318846227681280